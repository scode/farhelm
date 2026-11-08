## What this was about

Two Tensorlake deletion questions in Settings failed to express the user's answer or the operation's consequences.
Cancelling deletion under **Sandboxes not on this helm** could still allow a queued Delete click to destroy that
sandbox. Separately, opening a recipe editor during its first build could leave its later Delete question silent about
the snapshot that had since finished, even though deletion removed it.

Triage required cancellation to revoke the first question's authority immediately, and an unconditional recipe warning.
The maintainer considered Farhelm-built snapshots generated build output rather than user-owned work, but wanted the
question to say what happens.

## Things you should know

The sandbox question now accepts an answer only for the opening that displayed it. Cancel revokes that opening; an
earlier answer also cannot confirm a reopened question. Delete closes the question when accepted, and failed deletion
continues through the existing error and Retry cleanup path.

Recipe deletion always says: "Delete recipe X? Any running build is cancelled and the snapshots Farhelm built for it on
Tensorlake are deleted." This avoids relying on the editor's older view and distinguishes Farhelm's generated snapshots
from the user's supplied base image or snapshot, which deletion keeps. No backend deletion policy changed.

Both PRs include their execution records, feedback removal and changelog fragments. Their code, tests and explanatory
comments total 96 and 27 changed lines respectively, within each 100-line gate. There was no scope departure. Resource
monitoring stayed active with healthy samples and manual heartbeat checks; asynchronous alert delivery was not verified.

## Open questions and possible follow-ups

No maintainer decision remains. The cancellation regression is a headless test of the application's decision to accept a
Delete answer and prepare its deletion request; it does not dispatch browser events or delete real cloud resources. The
recipe checks use the existing simulated Tensorlake backend. No claim of live provider verification is made.

## The PRs

- [#3186](https://github.com/scode/farhelm/pull/3186/changes): honour cancellation of leftover sandbox deletion; change
  `nzwrlmsr`, bookmark `plan/tensorlake-settings-prompts/01-orphan-delete-confirm-slot`.
- [#3187](https://github.com/scode/farhelm/pull/3187/changes): state recipe deletion consequences unconditionally;
  change `qmqvryzz`, bookmark `plan/tensorlake-settings-prompts/02-recipe-delete-wording`, based on #3186.

Both are open drafts for the landing monitor.

## Checks run, reused and skipped

- Passed `cargo fmt --all -- --check` and `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`
  on each PR's final source, covering Rust syntax, types and lints.
- Passed the focused recorded Rust test with child command
  `cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(cancelled_and_stale_delete_answers_send_nothing)'`,
  run `d04f50ff-e82e-4ed2-a335-3d659b9aa62c`: one passed, 1098 selected out, four slots and zero retries on pinned
  nextest 0.9.148. Complete source identity was `e4192a52` plus the first PR's final working diff. This evidence is
  reused for final #3186 head `281a908a` and the stack: subsequent changes only added ledger URLs and the independent
  recipe warning; orphan admission code and its test stayed unchanged.
- Passed `cargo build` and the pinned-dx `dx build --package farhelm-ui --platform web --release` on the combined
  source, preparing the application used by browser validation.
- Passed the recorded browser selection
  `npx playwright test tensorlake-recipes.spec.ts -g 'a managed recipe is created|a recipe.s snapshot builds'`, run
  `779a2675-3696-41d2-b866-19c2e2ffc7c2`: four passed, two per Chromium and WebKit, zero retries or skips, with pinned
  tmux 3.7c. It checks the actual warning with no snapshot and with a Farhelm-built snapshot, and retains the user's
  base snapshot after deletion. Source identity was `281a908a` plus the second PR's final working diff; the later ledger
  URL amendment changes no executable input. Both successful runtime records have complete reports and output, observed
  EOF and no cleanup limitation.
- Passed the isolated test-delay checker (423 delays, no missing rationale), changed Markdown/TypeScript formatting,
  changelog format and instruction references. The instruction checker reported zero errors and three pre-existing
  citation warnings.
- Earlier clippy attempts exposed authoring errors in the first conversion and were corrected. Recorded Rust attempt
  `0f853888-772a-44e8-b0aa-ed46dc62bc4e` was interrupted during compilation once a remaining compile error was known:
  child signal 15, recorder exit 143, no executed tests or JUnit. Its complete output and source identity remain
  retained privately; it is not passing evidence or a latent flake. The successful focused retry is a separate
  observation.
- Skipped full Rust/browser suites, desktop execution, live cloud checks and CI: the focused admission regression,
  actual rendered warning in both engines, compilation and independent reviews cover the changes' identified risks.
  Main's intervening changes were only queue and TODO metadata; no code or spec interaction required a rebase or another
  runtime run.

## Review gate's outcome

Each PR received its required independent fresh-context gpt-6.1-sol/high adversarial review. The first found an old
UI-state read that prevented compilation; it was corrected before clippy and the focused test passed. The second had no
actionable findings. No finding was declined. Separate fresh cold readers accepted both commit and PR messages, and a
fresh resume checker reconciled the log, goal and local state after compaction.

### Landing

Landed with two fixes made at landing in #3186, one of them a browser test the change had broken.

In Settings → Tensorlake, under **Sandboxes not on this helm**, pressing Cancel on a sandbox's Delete question could
still let a Delete click the page had already queued go through, deleting the sandbox the user had just decided to keep.
Cancel now withdraws the question at once, and an answer given to an earlier question cannot confirm a newly opened one.
Users will also see one change in how the question behaves: pressing Delete now closes the question straight away, and
if deletion fails, the reason appears on the sandbox's row with **Retry cleanup**, where before the question stayed
open.

Deleting a recipe now always asks "Delete recipe X? Any running build is cancelled and the snapshots Farhelm built for
it on Tensorlake are deleted.", whether or not a build is running. Before, a question opened from an editor that was
started during the recipe's first build could leave out the snapshot that build had since finished, which deletion then
removed.

- #3186: the leftover-sandbox fix, its test and changelog entry, and the removal of the review finding it fixes from the
  review feedback queue.
- #3187: the recipe deletion wording, its tests and changelog entry, and the same removal for its finding.

#### What landed since the stack was based, and whether it interacted

Since this plan's base, main gained only a TODO.md entry (#3185), and this round landed no other plan. The plan's base
already included the recipe-bases plan (#3178), which reworked the recipe editor, recipe deletion and the recipe browser
tests just before. The landing monitor and, independently, a fresh-context review read #3178 against this plan and found
no conflict. The new recipe warning matches what deletion does after #3178: it cancels and waits for a running build,
then deletes only the snapshots Farhelm built for that recipe, and never the image or snapshot the user chose as its
base. Nothing in SPEC.md or SPEC_impl.md contradicts either change. The review also found the leftover-sandbox fix
complete: the question's Delete button is the only way to confirm it, the keyboard uses the same button, and a retry
after a failure goes through **Retry cleanup**, which opens a new question.

#### Fixes made at landing

- An existing browser test of the leftover-sandbox list ("Settings lists unknown Farhelm sandboxes and confirms snapshot
  cleanup", in the Tensorlake tunnels spec) still expected the question to stay open after a failed deletion. The
  executor ran only the recipe spec, so this was not seen before landing; the landing's check run failed that test on
  both engines. The test now checks the new behaviour (the question closes, the row reports the failure and offers Retry
  cleanup) and keeps its later steps unchanged. This adds a few lines of test to #3186, slightly past the plan's limit
  of about 100 changed lines for the executor's own work; a test kept in step with the change it covers was judged a
  landing fix, not more scope.
- The leftover-sandbox changelog entry said "a Tensorlake sandbox in Settings", which could be read as a host's own
  **Delete sandbox**. It now names **Sandboxes not on this helm**.

#### Things you should know

- After a failed deletion of a leftover sandbox, the reason now shows twice, on the sandbox's row and in the section's
  own error line, until the next action. Before, the open question hid the row's copy. A possible follow-up is to show
  the section's error only when the row is not already showing that failure.
- After pressing Delete, the row shows nothing new until the helm answers the deletion request; only then does it show
  "Cleaning up…" or the outcome, so a slow request looks idle at first. Before, the question stayed on screen meanwhile.
- The recipe question no longer ends with "This can't be undone". Deletion is still permanent; the report does not say
  whether dropping the phrase was deliberate.
- No browser test clicks Delete after Cancel. The cancellation fix is tested in the UI's Rust tests, which check that a
  cancelled or outdated answer sends no deletion request, without rendering the page.

#### Checks

These ran on the stack as delivered, stacked on main `525a2ab8` (its base plus one TODO.md entry), before the landing
fixes:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the UI's web-target clippy
  (`cargo clippy -p farhelm-ui --target wasm32-unknown-unknown --features web -- -D warnings`),
  `cargo check -p farhelm-ui --features desktop`, the check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`, 423 delays, none without a reason), `dprint check`,
  `python3 releasing/check-changelog.py format`, `cargo build` and the UI's release web build
  (`dx build --package farhelm-ui --platform web --release`): all passed.
- The UI crate's tests in its desktop build, through the test recorder
  (`python3 scripts/record-test-run.py --runner nextest --kind development --tmux none ...`,
  `cargo nextest run -p farhelm-ui --features desktop`, pinned nextest 0.9.148, four parallel test slots, no retries),
  run `319d220c-45b2-4060-84aa-b080e6dd904a`: all 1099 passed.
- The browser specs for Tensorlake settings, recipes and tunnels (`tensorlake-settings`, `tensorlake-recipes`,
  `tensorlake-tunnels`), on Chromium and WebKit (`--runner playwright`, `npx playwright test` with those three files,
  the pinned tmux 3.7c required, three workers, no retries), run `c97060e6-32de-4878-a695-243a2f6e8646`: 34 of 36
  passed. The two failures were the leftover-sandbox test described under "Fixes made at landing", on both engines: it
  found two error lines where it expected one, because the question it expected to stay open had closed. That run is
  kept as the evidence of the defect.

After the landing fixes (the test and the changelog wording, in #3186; no product code changed):

- The tunnels spec file on both engines, same recorder settings, `npx playwright test` with `tensorlake-tunnels.spec.ts`
  alone, against the same web build, run `87d511c5-0bbd-4b5a-a7ed-b11d84cb6c0b`: all 4 passed.
- `dprint check`, `python3 releasing/check-changelog.py format` and the test-delay check passed again.

- Reused: nothing from the executor's runs; the runs above cover its focused unit and recipe tests.
- Not run: the rest of the browser suite and the other Rust crates, the UI's JavaScript tests, a real Tensorlake sandbox
  and the native Mac app. Both PRs change only the UI's Tensorlake settings, which the runs above cover; the report
  notes that no live provider check was claimed.

Nothing in the executor's report became untrue at landing, beyond the PRs' draft state and heads: the landing fixes
changed #3186's head (and so #3187's), and both PRs are merged. The checks above also ran specs the report did not.
