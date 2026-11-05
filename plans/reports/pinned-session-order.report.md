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
