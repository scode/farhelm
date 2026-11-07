## What this was about

Starting a session in a managed GitHub checkout could make the user wait while Git consolidated Farhelm's repository
cache, with cleanup output in the session terminal. The maintainer chose to move that work to the existing background
worker, including when prepared checkouts are disabled.

Launch-time cache fetches, clones and prepared-checkout updates now disable automatic Git cleanup. After cloning ends,
the background worker either builds a needed spare, consolidating during its cache refresh, or consolidates the cache
alone. These modes share the existing login-shell helper and never run cleanup twice for one coalesced attempt.

## Things you should know

Cleanup can begin while a configured post-clone command is still running. It waits for cloning to end with prepared
checkouts enabled or disabled. A later launch can still wait if background cleanup already holds that repository's cache
lock; the maintainer explicitly accepted this tradeoff. No additional waiting-launch optimization was added.

The existing pack threshold, cache location and eviction age, prepared-checkout coalescing, cancellation and deadline
rules remain. Cleanup-only work does not extend the cache's last-checkout age. User repositories and their later Git
commands are unchanged. The live installation was not used.

## Open questions and possible follow-ups

No required work remains. Avoiding the accepted lock wait would require a separate scheduling change.

## PRs

- [#3130: defer checkout cache cleanup to the background](https://github.com/scode/farhelm/pull/3130/changes) — draft,
  commit `8edef7d72ae19297ccf8abec05e91ab3fc38ba89`.

## Checks run, reused and skipped

Runtime checks use the repository recorder, pinned nextest 0.9.148, four slots, zero retries and required pinned tmux
3.7c. Excluded tests are outside the selected scope.

- The supervisor selection passed 100 tests in `1557d00d-8db3-4376-9438-762e0306cc28`:
  `cargo nextest run -p farhelm-supervisor --lib -E 'test(/^launch::/) | test(/^spare_take::/) | test(/^spare_build::/) | test(/^service::spare_builder::/) | test(spare_builder_)'`.
  It includes the real-Git accumulated-pack proof, launch and spare-update overrides, cache-lock and eviction-age proof,
  disabled-spare clone gate, and existing coalescing/publication/cancellation checks. The recorded working tree was
  based on `bf9664c5`, source fingerprint beginning `7c6d3895dc256370`; the pushed revision has identical Rust behavior.
  Later edits only clarified specification prose.
- The shipped helper and managed-checkout selection passed 18 tests plus fixture setup in
  `ca563242-4069-4028-a913-e42dd5965fd1`:
  `cargo nextest run -p farhelm --test spare_builder --test e2e -E 'binary(spare_builder) | test(/^github_checkouts::/)'`.
  It covers real guarded cleanup-only success/failure, login-shell receipt refusal, owner-death and deadline cleanup,
  ordinary GitHub checkout creation/replay, and taking and replenishing prepared checkouts with the frozen switch. Both
  recorded runs have complete JUnit reports and retained output, matched the pinned substrate, and contain no runtime
  `SKIPPED` notices. Their Rust source matches the pushed revision.
- `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
  passed, covering tests and the shipped CLI configuration.
- `cargo fmt --all -- --check`, dprint on changed Markdown, changelog format and diff whitespace checks passed. The
  pinned source-only test-delay checker inspected 420 delays with none missing a rationale.
- The initial compile failed with a missing Rust error-type annotation, before any runtime test ran, in
  `8832374c-1080-4631-b3fa-fb8b964f2251`. The annotation was corrected before the passing supervisor run. Failure
  evidence is retained; this was a same-session compile correction, not an established latent flake.
- No earlier plan's runtime evidence was reused. The full Rust battery, browser and UI tests, native desktop,
  installer/release suites and GitHub CI were skipped because this change is confined to checkout preparation and its
  existing background helper. The selected supervisor and shipped-process checks cover those paths. No UI code changed.
  No instruction, heading or path was renamed, so the agent-instructions checker was not needed.

## Review gate

One fresh-context gpt-6.1-sol reviewer at high effort reviewed the complete change against the plan and the repository's
test-authoring checklist, supplied verbatim. It found no actionable issues. A separate documentation pass covered every
touched file. The final PR text passed a valid fresh-context cold read after its first title's timing claim was
corrected.

### Landing

Landed with one guide-page sentence fixed. Starting a session in a managed GitHub checkout could make you wait while Git
tidied Farhelm's cache of that repository, with Git's cleanup output in the session's terminal. That cleanup now runs in
the background after the launch.

- #3130: the change, its tests and the GitHub checkouts doc.

#### What landed since the stack was based, and whether it interacted

Since this plan's base, main landed the named-credentials work for Tensorlake hosts (#3074 to #3126). It changed the
same background worker this plan reuses, the one that prepares a checkout of a repository ahead of its next launch: that
worker now passes a selected GitHub token to its clones. A fresh-context review traced the two together: the token
reaches the background cleanup as well, which is harmless because cleanup does no network access, and the credentials
work's new test of a private checkout runs through the code this plan changed; it passed in this round's workspace run.
Of the two other plans in this round, the session-row hover change landed first and does not touch this code, and the
path-link fix was sent back to its executor; this plan was moved off it before landing, so nothing of it is in what
landed.

#### Fixes made at landing

- The Start a session guide page still said an occasional launch takes longer while Git consolidates the cache. It now
  says Git tidies the cache in the background after a launch, and that another launch of the same repository may wait
  while that finishes.

#### Things you should know

- A failure of that background cleanup is logged with the existing message for a failed checkout prepared ahead of time,
  which also says the ordinary clone remains available. The wording fits cleanup only loosely; it changes nothing for
  users.
- This plan stops the cleanup in Farhelm's own Git commands. If a post-clone command you configured in Settings runs Git
  commands of its own, their cleanup can still show in the terminal; a possible follow-up if that is ever seen.

#### Checks

This round's checks ran once on three plans stacked on main `844a7209`: this one, the other plan landing in this round,
and the path-link fix that was then sent back. Before the landing fixes:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the lint of the UI's browser build,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 420 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed.
- Every Rust test in the workspace, `cargo nextest run --workspace --exclude farhelm-desktop` (with rootless Podman and
  its offline image present, so the containerized tests ran), run `9d012e5b-959d-47a6-b478-c48fc1463eaf`: all 5734
  passed, 69 skipped by their own design. The desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`,
  run `6f70cd74-34ec-4007-aa21-eca5d055113b`: all 1089 passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `3f1b635d-ea54-47a6-af2d-115fbfe321d4`:
  527 passed, 2 failed, the two stylesheet tests the session-row hover plan's ring color broke (fixed at landing).
- The grid terminal component's browser suite on Chromium and WebKit
  (`npx playwright test --config grid-client/playwright.config.ts` in `e2e/`), run
  `327273ed-ec2f-4f31-ad50-d1f5f5666888`: 184 passed, 2 failed, one container-session path-link test on both engines,
  caused by the path-link fix (it passed without that plan, run `afd16173-4ead-44a8-8fbc-187689c5821d`), which was sent
  back to its executor.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each ran its share of the suite's spec files through the test recorder,
  `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, built locally; the sandboxes run Node 24.16 rather
  than the development machine's 24.20), runs `bee8fe6d-ae5f-492b-a9cf-438692919ea1`,
  `84e886a6-e907-400e-9c9f-22f7f8503002`, `5fb6cc43-53ae-4eb1-bd6e-f9c26451aaf8`,
  `57f343a5-cd3d-44b8-9949-7b169a3102ef`, `de04602a-31b5-413e-a9f5-6ab042b7551c`,
  `600ddcc3-e6fb-49e0-938e-d2d2e0ae8bda`, `3ad00bf1-15f7-4bd0-9a5d-52dde95d1a58`,
  `355178cf-8854-4faa-b7bd-4fdbdd121568`, `911472df-570e-4ff5-a133-8a053f9a96e2`,
  `150b5c57-6706-441a-9053-43e80b235a42`: 3018 passed, 14 skipped by the specs' own design, 2 failed, both on WebKit in
  run `600ddcc3`. "A pending checkout folder save survives closing settings" is a recorded flake with the same
  signature. "An idle session already seen uses its row's status foreground with the plain word" saw a white status dot
  once; it passed three re-runs on each engine after the fixes below. Then the suite's old-terminal set (the tests that
  only apply to sessions still in old terminals) on four sandboxes with the same build, runs
  `994df3b7-e0a0-4bb7-a565-58654f5db524`, `f8cb1564-f01f-425e-93f0-5fccb47ee5cf`,
  `21865731-ecc8-4245-9caf-a82fdcc06a42`, `e0baf14f-7391-413d-80f8-f46cbd2c2734`: 107 passed, 1 skipped by its design.
- After the landing fixes, with the path-link fix taken out of the stack: the UI's JavaScript tests, run
  `6f6f66b1-643c-4e2c-955d-ba1f43442cf5`: all 526 passed (three fewer than before: the path-link fix's own tests went
  with it); `dprint check`, the changelog format check, the docs website build, the workspace debug build and the UI's
  release web build: all passed. On Tensorlake sandboxes with a new build: the tooltip spec file on Chromium and WebKit,
  runs `9dc24879-9764-4565-9786-43b5592b924d` and `8df616f4-2fd7-4833-a96f-7f553e645a82`: 17 passed on each engine; and
  three repeats of the idle-session status test on each engine, runs `8fef0602-cb27-4a2c-a3a7-82eeefe4be15` and
  `2709932a-61e9-4c63-a6dc-2d2562ebd3da`: all passed.
- Reused for the landed tree, from the runs before the fixes: the lints, the Rust and desktop-build tests, and the rest
  of both browser runs and the grid terminal suite. The fixes rewrote three stylesheet rules with the same colors and
  changed SPEC.md and a guide page; taking the path-link fix out removed only its terminal link code, its tests and its
  docs. The grid suite's only failures were that plan's, and nothing that landed touches the grid terminal.
- Not run: the doctests (no doc examples changed), the desktop smoke tests and the installer tests (no desktop shell,
  packaging or installer code changed), the docs website preview (the build above passed; no page was viewed), and the
  native Mac app (not runnable on Linux).

Nothing in the executor's report became untrue at landing.
