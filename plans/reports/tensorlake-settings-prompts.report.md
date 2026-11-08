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
