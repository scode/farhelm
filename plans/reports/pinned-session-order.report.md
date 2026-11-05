## What this was about

Pinned sessions used to follow the sidebar's chosen sort. The requested behavior is one shared order you can choose,
with new pins at the bottom, dragging within the pinned group, and Move up / Move down in each pin's menu.

The three-PR stack implements that behavior. Unpinned sessions keep the chosen sort, filters still hide pins without
changing their saved order, and the quick switcher keeps its activity order. Replace and Replace with carry the pin's
position to the replacement; unpinning and pinning again puts it at the bottom.

## Things you should know

The helm database advances from schema 73 to 74. Existing pins start oldest session first; pins whose creation time is
unknown follow. This upgrade and the new shared order are stated in the first PR's description.

Pins already in the list's held order can move while the pointer remains over the list. A pin added during that wait
gets its grip and Move actions when the hold ends, including a new pin already beside the earlier pins. Leaving the list
releases the hold immediately; otherwise it takes five seconds of pointer stillness with the row menu closed.

Dragging uses the grip, with a lifted surface and drop line. Escape, capture interruption, an outside release, source
removal or list teardown cancels without changing pin membership. The menu actions remain the keyboard alternative and
return focus to the moved row's menu button. The implementation uses the existing shared move and refresh path, with no
private ordering or new write queue.

The Read the session list guide now describes the order, controls and hold exception. This was an incidental docs edit
in an unattended run, so no preview or screenshot publication was started. The addressed TODO entry is removed, and the
same unreleased changelog fragment covers menu and drag controls.

## Open questions and possible follow-ups

No product decision remains unresolved. Dragging in the desktop app itself was not exercised; native Mac dragging still
needs a devbuild check. Chromium and WebKit provide the pointer-interaction evidence below.

## The PRs

- [#3041](https://github.com/scode/farhelm/pull/3041/changes): shared pin positions, migration and relative moves.
- [#3046](https://github.com/scode/farhelm/pull/3046/changes): manual sidebar order, the hold exception and menu moves.
- [#3047](https://github.com/scode/farhelm/pull/3047/changes): grip dragging, integration coverage, specs/docs and TODO
  removal.

All remain open drafts; this executor marked none ready and merged none.

## Checks run, reused and skipped

Runtime commands used the recorder in development mode. Rust selections used pinned nextest 0.9.148, four global slots
and zero retries. Browser runs used pinned tmux 3.7c, the suite's three owned stacks and up to three workers, zero
retries, on Chromium and WebKit. Complete source, report, output and cleanup records were inspected; no runtime
substrate skip is counted as coverage.

- Current pin browser coverage: 10 passed, five per engine, recorder `d3843312-013c-491e-9692-ce3d5354eab0`, running
  `pins.spec.ts`. This checks both menu directions and focus, held new-pin eligibility, real pointer drops, exact
  relative requests, shared/reloaded order, cancellation, source removal and successful dragging after listing
  error/recovery. Both retained drag screenshots were inspected. It tested the working tree above PR2
  `004b0b8a57100233629e5b38381e14d882e0c1bd`, source fingerprint
  `0afaa24214567b0b97e03e45a1de473b3931802d4b965047ec20d23e42467316`.
- Current adjacent browser coverage: 58 passed, 29 per engine, recorder `2b8dd896-c7f3-4d44-bf38-5d3f65cf215b`, running
  all `sort.spec.ts` cases and eight selected sidebar menu/focus/geometry cases per engine. This exercises the
  drag-enabled list's existing hold handlers and checks that grips preserve row gutters, menu placement and keyboard
  focus. It tested the same revision and fingerprint as the current pin run above.
- Current helm storage, migration and route selection: 17 passed, 1479 selected out, recorder
  `ad188e39-d4ec-4d7d-9b99-2fd22c8d8623`, using `cargo nextest run -p farhelm-helm --lib -E <pin-related selection>`.
  This covers relative moves, feed notifications, rollback, hidden pins, Replace inheritance, append/repin behavior,
  historical migrations, schema 74 and the listing contract after the rebase. It tested the same revision and source
  fingerprint as the current pin browser run; no tmux or live-install substrate was needed.
- Focused UI Rust: 92 passed, 776 selected out, recorder `82e405ac-fc73-4f63-90ed-cf1fd51dc7c7`, covering list-view,
  row, order/hold, bridge recovery and asset-inventory tests. Focused drag JavaScript: eight passed, recorder
  `3d3abb77-dc98-4dc4-92a4-6b74f8a6e138`. These parent-run checks tested the final drag source before rebase, on PR2
  `c8df2dd091ccad39db48919a105ec58ddecd44ff`, fingerprint
  `bae84f9972d1312270216394322efbd1b87d4e45fe7c724baffd8ce2752ff3c1`. All three PR interdiffs were empty after rebasing
  onto `5065e022`; upstream changes do not alter these ordering or bridge contracts, so these checks are reused.
- Reused PR2 regression run `110f0195-28d1-4de4-8f88-9be33d98a75a`: 152 passed, 76 per engine, covering all sort and
  Replace cases and the affected menu/focus cases. It tested PR2's working tree above PR1 `8f0202d3`, fingerprint
  `abb95b79bf743c6f2da01c64fb8c25d33b798a845a449d93924433dae5867ac4`. Replace's mutation and pin-inheritance paths are
  unchanged; current pin and adjacent checks cover the new list-root wiring. The 18 menu-focus JavaScript scenarios also
  passed in `f68823ec-ad8c-4ff7-81d4-a45338e169ff`; that helper is unchanged by dragging and the rebase.
- Reused `cargo clippy -p farhelm-helm --all-targets -- -D warnings` from the PR1 working tree on base `9252585b`,
  fingerprint `db08433de561c725875ceb908c30699fc1a840e34ba39a5f8f7190b3e7bc57b9`, and the corresponding UI all-target
  Clippy command from the final pre-rebase drag tree identified above. The reviewed patches survived the rebase
  unchanged; upstream changes do not alter their pin-specific contracts. Current Rust formatting, changed Markdown
  formatting, changelog-fragment format and whitespace checks passed. The final isolated test-delay checker inspected
  412 delays with no missing rationale. Fresh rebased CLI/fixture, release web and native desktop builds passed.
- The actual generated desktop binary's `--print-assets` exited successfully in recorder
  `5bcf95a1-8244-43f5-8f1c-e2bb25d94e61`. `check-desktop-assets.sh --compare <fresh-web-bundle> <requested-assets>`
  passed in `def3eecb-e905-4ca0-ad24-153349c9bea8`: all 39 requested assets equal the web bundle's product assets,
  including the new drag helper; its self-test rejects divergence in either direction. Both used the current pin browser
  source identity above. This checks generated asset paths; it does not launch the native app.

The initial full UI attempt `b2ec8171-e100-47f1-a8e3-b260b1eebd9b` had 861 passes and three failures in the new test
fixtures. Those fixtures were corrected and checked narrowly; the later 88-case PR2 and 92-case PR3 parent runs passed.
The full command was not rerun, and is not reported as a full-suite pass. Initial compile failures
`cae47b43-9229-4ff8-a55e-cdc7bd84cd24` and `89e91183-3efd-41d9-9490-65b667abe60f` were corrected before the successful
focused checks. Browser run `a5e05ed5-3c84-44cd-94ff-8d072507f3a5` exposed menu-toggle focus loss in both engines; the
existing focus helper was repaired and the final pin/menu checks passed. All failed UUID evidence was retained; these
same-session observations are not latent-flake entries.

The careful rebase read every upstream diff. Permission durability and retry-key changes preserve the database call
contract used by pin writes; terminal tab/listing repairs preserve the session shape and helm pin projection. Grid-link,
approval-card, browser-fixture, release-illustration and bookkeeping changes do not alter pin ordering. No conflicts or
design changes were needed, and each reviewed PR patch was preserved.

The full Rust, JavaScript and browser batteries, desktop smoke, installer/provisioning tests and CI were skipped: the
selected store, route, list/gesture, browser interaction and asset checks cover the changed contracts. No website build
or instruction-reference check was needed because the prose edit changes no links, headings, frontmatter or instruction
file. No live installation was changed.

## Review gate's outcome

Each PR passed its required single fresh-context gpt-6.1-sol review at high effort. The menu focus findings were fixed;
the bounded repair in the existing helper also passed an independent scope assessment. At the drag writer's second
checkpoint, the parent corrected list-root ownership so recovery cannot let old bridge cleanup disable a replacement;
native and real-browser regressions now cover it. The final drag review found no actionable issue. No required finding
was rejected or left unresolved. Commit and PR wording passed independent cold reads, and every changed source/test file
received a separate documentation pass.

### Landing

Landed with fixes at landing. Pinned sessions now keep one order you choose: new pins go to the bottom, and you can drag
a pin within the pinned group or use Move up and Move down in its menu.

- #3041: the helm stores each pin's position (helm database schema 73 to 74), with a test fix from the landing.
- #3046: the sidebar shows pins in that order and the Move actions, with a test fix from the landing.
- #3047: dragging pins, with the landing's drag fix.

#### The fixes made at landing

The drag's drop area was bounded by the whole sidebar rather than by the part of it that scrolls. The sidebar also holds
surfaces above the scrolling list: the app bar, and since #3044 (which landed after this plan's base) the host attention
list, up to 280 pixels tall. With the list scrolled so that the first pins were out of sight, releasing a dragged pin
over one of those surfaces counted as a drop inside the pinned group: the pin moved next to a row the user could not
see, instead of the drag being cancelled as the specification says. The bug was in the plan's own code over the app
bar's narrow band; the attention list made it much easier to hit. The landing bounds the drop area by the scrolling list
as well and corrected two code comments that named the wrong element as the scroller (one from this plan, one older that
probably misled it). It added a case to the drag's JavaScript tests that fails on the plan's original code and passes
with the fix. No browser test drags with the attention list showing; the JavaScript case covers the geometry. It also
fixed a smaller slip the review found: a grip that cannot drag (while the row is busy, renaming, deleting or confirming)
still appeared on hover, because the rule hiding it was less specific than the rule revealing it. Both are in #3047's
commit and were in place before every check below.

The landing's full test runs then found two failures the plan itself caused, in tests the executor's own narrower
selections did not include. The tooltip script's JavaScript tests use a fake page element that refuses any selector they
do not model, and #3046's menu-focus change now asks whether a focused element is a session row's menu button; the
landing taught the fake that question (a test-only change, in #3046's commit). A helm test of the Tensorlake schema step
expected a migrated database to end at schema 73, the newest version when it was written; the landing made it compare
against the helm's current version instead, so it no longer breaks at each schema change (in #3041's commit; the version
constant is now visible across the helm crate, with no behavior change). Neither changes what ships, so the browser runs
below, made before this second fix, still describe what landed.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained terminal-text-drops (#3029, #3045: the vendored terminal code, the terminal
emulator's tests and the terminal documentation) and host-attention-list (#3044), which interacted as described above.
#3044 also changed the same session list view and stylesheet; a fresh-context review found those edits fit together
otherwise, and that opening Hosts (including from an attention row) cancels a drag. Nothing else landed in this round.

#### Things you should know

- The helm database's move to schema 74 has no downgrade path: a helm older than this change refuses to open the
  database, as with every helm schema change. SPEC_impl.md's new schema entry does not say so, as the other entries do.
  The last release (v0.35.0) is at schema 69, so main already could not be downgraded to it; this adds no new loss.
- The browser test of hover text fails on main every time, because the Add host form's "SSH" and "Tensorlake sandbox"
  choices have no hover text (from #2973). Nothing in TODO.md or the review queue records it yet. This plan's new grip
  has hover text.
- Dragging in the Mac app itself was not exercised, by the executor or the landing; Chromium and WebKit are the
  evidence.

#### Checks

This round landed only this plan, stacked on main `018c691f`. The checks ran on the stack with the drag fix (top
`46088b77`), and those touched by the later test fixes ran again on the final stack (top `90fcbc1e`):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` (both again on the final stack), the
  web-target UI lint, `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  the check that every test delay states its reason (`python -B scripts/check-test-sleeps.py`, 412 delays, none without
  a reason), `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no
  errors; three warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug
  build and the UI's release web build: all passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `248cf6b1-9359-492d-bc8c-b5105baa2482`:
  524 passed, 3 failed, the tooltip tests described above; on the final stack, run
  `a92645ea-0ded-408a-9195-749fd3e2a9e0`: all 527 passed. The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run
  `3f6e38e2-a1ef-4df8-adeb-a3d2f6016e29`: all 873 passed; its desktop-build tests,
  `cargo nextest run -p farhelm-ui --features desktop`, run `848b5f17-6d88-4f93-bf8e-f77bd1707bbe`: all 1062 passed.
- The whole Rust workspace with the pinned tmux, `cargo nextest run --workspace --exclude farhelm-desktop`, run
  `b660ba55-d18b-40c8-a7d4-ba118ac1ccc3`: 5626 passed, 70 skipped (ignored by design), 1 failed, the helm schema test
  described above; on the final stack, the helm's tests, `cargo nextest run -p farhelm-helm`, run
  `ee42a6c3-46cf-48c1-bb20-7e09971cc0d2`: all 1496 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  the stack with the drag fix), runs `d13376a5-68df-4ef6-8523-1e6a896103aa`, `33242401-527d-4d46-a083-d46723512d73`,
  `a2a420cf-cf13-4685-8291-ef9b5776d3b3`, `f14c83fa-5bfc-49cc-b78f-415857edbd33`,
  `ef19a386-2881-4ab6-be88-9f0b5d053148`, `ff511eae-2e84-4db3-a22a-54bcbfba9b03`,
  `4d9291ef-904c-4048-ae81-b527a60f6449`, `f8f35e89-4ff3-42b9-bc27-2827925bcecb`,
  `8c18b9e2-fff8-4ffc-ba56-24c10667acdc`, `6dafd212-224f-4127-9064-4d421d4ca1e2`: 3007 passed, 18 skipped by the specs'
  own design, 9 failed. Then the same suite with every stack's terminals in terminal hosts (this run covers only the
  specs listed for host terminals in `e2e/host-terminal-specs.ts`, so it holds fewer tests), runs
  `ef7a7ac9-d776-43ca-852f-8a543fa7b0b1`, `8e1449d1-ea15-4e0a-a653-3ca2ec5ddd90`,
  `be7435f7-0a6e-48bb-8069-37967a16d839`, `0eac39ca-a5a9-4b54-ba80-56e7ab43efba`,
  `11b4495a-b3b3-4406-a659-891850c74446`, `4a3923dc-60c1-467e-993f-dafd35334e43`,
  `14bd9830-0b3c-4ffa-8119-ce7ece64c798`, `f722b3b4-6fbc-45e4-9120-7380a3f88b8a`,
  `b9844f63-7503-466b-a3d0-bc534388e186`, `4a84ad15-a21e-455a-939a-49bfea8e981c`: 2083 passed, 75 skipped by the specs'
  own design, 2 failed. Of the eleven failures:
  - "Sidebar surfaces give every control hover text" failed four times, in both engines in both suites (runs `a2a420cf`,
    `f8f35e89`, `0eac39ca`, `f722b3b4`), naming only the Add host form's "SSH" and "Tensorlake sandbox" choices, which
    have no hover text (see Things you should know).
  - "Post-clone commands with carriage returns refuse edits but can be cleared" (WebKit, run `ff511eae`), a flake
    recorded by open draft PR #2549.
  - Six single failures in the ordinary suite, none in pins, the session list or the attention list, each of which then
    passed three re-runs on the same build, also on sandboxes (the run ids below are the re-runs): "keeps a number popup
    above a bolt transfer" (WebKit; a 60-second wait for an intercepted save to finish, like the recorded WebKit
    pending-save flakes; failed in `f14c83fa`, re-run `55db623b-9ee8-420c-b370-8b0fa63619ba`); "suspending a current
    idle agent acts immediately" and "an empty cached listing asks when the host disconnects during the read" (WebKit;
    the first found its session missing from an activity read, as another test in the same spec did in an earlier round,
    the second had a request route answered twice; failed in `f14c83fa`, re-run `399aa780-8b13-47da-b46d-584d26340128`);
    "a first identity learned while following: replacement" (Chromium; an update's progress label showed no step name
    within 5 seconds, as a similar provisioning test did in an earlier round; failed in `4d9291ef`, re-run
    `a2bc56b8-f28c-4c1e-b7f6-d4a1b21ad011`, which also ran that test's sibling); "host-settings scrollports stay
    reachable while saving" (WebKit; a scroll area briefly had no keyboard stop; failed in `8c18b9e2`, re-run
    `49b4fdd8-e894-41fc-91e4-001e689c26f3`); and "terminal link ownership opens once and preserves program clicks in all
    mounted terminals" (WebKit; a terminal's fake agent did not print its ready line in time; failed in `6dafd212`,
    re-run `021948e9-7d60-435e-b539-2e50c4579039`). Each is a single failure, not recorded as a flake.
- Not run: the doctests (no doc examples changed), the grid component's browser suite (no grid code changed), the
  desktop smoke tests and the installer tests (no desktop shell, packaging or installer code changed), the development
  screenshot review (no shot changed), the docs website preview (the build above passed; no page was viewed) and the
  native Mac app (not runnable on Linux), so dragging in the Mac app is unverified.

The executor's report calls the PRs drafts and says a release outside the pinned group cancels the drag, which its code
did not do for a release over the surfaces above the list; the landing's fix makes it true. Its argument that narrower
selections covered the change, in place of the full Rust and JavaScript runs, did not hold: the full runs found the two
test failures above. Its browser, UI and JavaScript runs predate #3044; the landing's runs in Checks above include it.
What landed is its commits rebased onto current main with the landing's fixes, each in the commit it belongs to. Nothing
else in it became untrue at landing.
