## What this was about

The website linked to Farhelm's Discord, but someone in the app looking for help had no direct route there. The plan
adds **Chat on Discord** immediately after **Send feedback** in the **?** menu, and the line "Rather talk in the open?
Chat on Discord" below the feedback dialog's summary. Both open the same invite in the browser, using the existing path
for Documentation.

## Things you should know

Following the dialog's link leaves the dialog and anything typed in it open. It stays in the header, before Close in
keyboard order, so Tab from Close still reaches Message. The menu retains its other items' order, with the desktop
update check last. Both links have hover text.

The updated docs page is `website/src/content/docs/docs/using/send-feedback.md` (`/docs/using/send-feedback/`). No
preview was started during unattended execution. Published release screenshots and their tests remain unchanged; only
the development visual-review helper's menu count and comment changed.

## Open questions and possible follow-ups

None required for this plan.

## The PRs

- [#2963](https://github.com/scode/farhelm/pull/2963/changes): Discord links in the help menu and feedback dialog, head
  `b905b8e0`.

## Checks run, reused and skipped

Run on Linux for this PR:

- `cargo nextest run -p farhelm-ui --lib`, through the recorder with pinned nextest, four slots and zero retries: 838
  passed, none skipped; run `c4baf5d9-d0fd-4802-acf1-6d98622f3d00`. This includes the menu order with and without the
  desktop updater. The only subsequent source correction was in a browser keyboard test; the Rust tested here is the
  committed Rust.
- The browser selection below, through the recorder with three workers, one temporary stack per worker, pinned tmux and
  zero retries: 40 passed, 20 in Chromium and 20 in WebKit; run `93d17e40-441e-44f2-9325-56110e00cf84`. It covers both
  new opener calls, retained draft text, menu order and keyboard navigation, the menu at 240px window height, feedback
  layout, tooltips, and keyboard scrolling including the sent dialog at 360x220.
- The host-terminal browser selection below, with `FARHELM_E2E_SUITE_TERMINALS=host` explicitly retained by the
  recorder: four passed, two per engine; run `c3aa3cf0-dfe1-4549-98c8-d2877251035d`. The suite checks that its temporary
  stack actually uses host terminals.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo fmt --all -- --check`, changed Markdown `dprint check`, `python3 releasing/check-changelog.py format`, and the
  isolated `python -B scripts/check-test-sleeps.py`: passed. The delay checker inspected 397 calls with none missing a
  rationale. This repository has no TypeScript or CSS dprint plugin; no formatter coverage is claimed for those files.
- `bun run build` in `website`: 35 pages built, all internal links valid. The release web build and matching
  `cargo build` also passed. The first web-build invocation resolved an unrelated command named `dx` and stopped before
  compilation; using the installed Dioxus CLI produced the successful build.

Default-terminal browser child selection, from `e2e`:

```sh
npx playwright test 'feedback\.spec\.ts|tooltip-coverage\.spec\.ts|filters\.spec\.ts|keyboard-scroll\.spec\.ts|modal-layout\.spec\.ts' -g 'feedback|help menu|rail Help menu|sidebar surfaces give every control hover text|sidebar menu flyouts|opens its page'
```

Host-terminal child selection, also from `e2e`:

```sh
npx playwright test 'keyboard-scroll\.spec\.ts' -g feedback
```

All three runtime records have complete source and runner reports, complete retained output under the recorder's 8 MiB
output cap, no runtime `SKIPPED` markers and no forced cleanup. No runtime test failed. The browser runs cover the
committed product and test files. The narrow and short-height screenshots were also inspected: the public link sits
below the private summary and the actions remain reachable.

No earlier-session checks were reused. Main changes through `2e2d60d1` were inspected: a separate terminal-hyperlink
fix, TODO edits and planning bookkeeping. They change neither the shared external opener nor help/feedback layout or
keyboard behavior, so no rebase or additional runtime run was needed.

The whole Rust workspace, full browser suite and unrelated cases in the selected files were skipped because these
focused checks cover the changed behavior. The staged-fleet visual-review run was skipped: only its count and comment
changed, and the same real web menu count is checked above. Native desktop execution and macOS were not tested; the
desktop feature compiles, and both new links reuse the unchanged system-browser path. No published screenshots were
refreshed. The instruction checker was unnecessary: no instruction file, Markdown heading or file path changed.

## Review gate outcome

The required fresh-context gpt-6.1-sol high reviewer found one affected keyboard test whose old focus origin no longer
preceded the scroll region after adding the enabled Discord link. The test now starts from that link, preserving its
pending-send scrolling and refusal-recovery assertions. The same reviewer checked the correction and reported no
outstanding findings. The focused browser runs passed afterwards. Commit and PR wording received a separate
fresh-context cold read, with no contradicted claim or convention issue.

### Landing

Landed with no fixes of its own, third of the four plans landed together in this round. The **?** menu has **Chat on
Discord** right after **Send feedback**, and the feedback dialog has "Rather talk in the open? Chat on Discord" below
its summary; both open the same invite in the browser.

- #2963: the menu item, the dialog line, their tests, the specification, and the Send feedback docs page
  (`/docs/using/send-feedback/`).

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the grid terminal's link fix (#2949), recovery of unavailable terminal hosts and tabs
(#2950, #2956), the orderly stop's checkpoint changes (#2952, #2954), and PRs that only added TODO entries and plans;
none touches the help menu or the feedback dialog. In this round, colored-harness-marks also edits the app's stylesheet;
a fresh-context review read both against each other and found no overlap. The rebase conflicted only in TODO.md, where
main had changed entries near the one this plan removes; the landing kept main's entries and removed this plan's.

#### Things you should know

- The feedback dialog's recorded Send-color flake failed once in this round's browser suite, on Chromium; the checks
  below say why it is not this plan's.
- The documentation screenshots still show the six-item help menu until their next refresh. The development screenshot
  review's menu count was updated by the plan; that review was not run, since the feedback tests assert the same count.

#### Checks

This round landed faster-snapshot-encoder, colored-harness-marks, discord-link-in-app and grid-drop-and-drag-notice,
stacked in that order on main `422b081f`. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 403 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older reports), the docs website build (`bun run build` in `website/`), the workspace debug build and
  the UI's release web build: all passed. The landing's comment fix in grid-drop-and-drag-notice came after these; the
  web build was redone with it before every run below.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `cb6b828b-e7ae-49a7-b987-94ce69ca76ff`:
  all 512 passed. The whole grid-client browser config on Chromium and WebKit
  (`npx playwright test --config grid-client/playwright.config.ts --workers=3 --retries=0` in `e2e/`, through the
  recorder's generic mode), run `370b2ec1-b2a6-4a36-bcb9-10f5d6be7d5f`: all 156 passed. The UI's Rust tests,
  `cargo nextest run -p farhelm-ui`, run `9babfbb6-a2cb-40e6-bb2e-a12424c728a8`: all 838 passed; its desktop-build
  tests, `cargo nextest run -p farhelm-ui --features desktop`, run `8656b073-6e6d-4eb0-b447-f6bcfc5cd7f3`: all 1027
  passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, no retries, on a machine with rootless Podman, run `1a6fcbb0-2016-46a8-81da-c493590d334f`: 5416 passed, 70 left
  out by the test runner (tests marked ignored), 4 failed. "Shallow pause resumes without reset or replay" is a recorded
  open flake. The other three are the ones the previous round found failing the same way on main without its plans
  (stop-checkpoint-burst's landing notes name them: two restart-gap listing tests and an OMP report test, in code none
  of this round's plans changes); they failed the same way here, in a second round now. They are recorded nowhere yet;
  the maintainer was told in conversation, and finding their cause is still an open follow-up.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack; Node 24.16 there, against 24.20 locally), then the browser suite's host-terminal run (every stack's
  terminals in terminal hosts, `FARHELM_E2E_SUITE_TERMINALS=host`) the same way. A first pass (full suite
  `ef3d153c-7523-4ea9-9fe4-49255aef0b45`, `73957c44-c11a-4b7a-ad61-ffc308bbe92a`,
  `7f0594d3-378e-4e58-a15c-557beacd6559`, `6ed08407-260d-4787-bc05-072320c5b339`,
  `ccbaf56f-4d47-427a-9e37-a82823f24ce5`, `a50e8639-2453-4ec3-b986-549a7d3b42c5`,
  `d1880e6c-4f48-41ba-bf34-0d0d26b02432`, `b70624ce-c46e-49b3-837b-d91bfe79d5ad`,
  `0547f105-b764-491a-85df-2ea4ab31b443`, `9c23218c-d538-402e-9281-f742f9ee9c92`; host-terminal run
  `631fa91e-3e5e-4ce3-ad55-f7d0a35ba2de`, `05ef7b5c-1295-4f4e-9792-916123999aec`,
  `d171d25b-3664-437f-b7df-2c4efa4e3bf0`, `48da66bd-1752-4f03-819f-543c859debb0`,
  `f3c33b24-3498-48af-affb-06af4a7b5da2`, `5aa981c0-e80c-4c66-8fa0-334939df9756`,
  `9094e996-a8e8-466c-ba89-3817343ad3a8`, `e7df97ce-6936-489a-9da9-37a3e5ece567`,
  `d74c9381-172b-4938-ae06-7d3f0bee1951`, `d646a14a-c66a-4682-9573-39477fb48427`) is not evidence: every sandbox's setup
  check reported that ssh to the machine itself failed, so the suite's second, ssh-reached host never connected and
  dozens of tests that use it failed. The cause was in the sandbox setup (port 22 there is answered by a service that
  refuses the test user's key); the fix, in the sandbox shard script, was in place for everything below and lands as its
  own change after this round.
- The second pass, on the same build, with ssh to the machine itself working on every sandbox. Full suite (runs
  `4251f5d4-5ecc-40c8-a1eb-7d20b5cc33ac`, `a782f3c9-f279-439e-80a0-e84eb3afc879`,
  `1a264a30-cd27-4f6e-bd2c-f3c3853fe0b6`, `abb8dc84-2070-4352-86c2-f5c4b7b4a272`,
  `e1ea38c4-f840-4bc6-8813-ab0c139a5b56`, `efb7587f-c71e-4756-9404-acadcf7390e1`,
  `d3b9327c-fb12-499d-9436-660d8cc369b6`, `fb86dd17-8394-454f-8f4c-3aebc6279b6e`,
  `9da0db7d-f592-483b-ae00-f2ed489e3a40`, `85f3947a-4cd8-40bc-aa1a-e7f72e70554d`): 2860 passed, 16 skipped by design, 4
  failed. Host-terminal run (`2ebb4d8a-185f-4845-8543-2c226689085f`, `3a43b91b-298b-44e1-a344-7b44415e76b0`,
  `dcd26c51-6225-4d8a-9b82-2acc8c751b07`, `434370fc-d4af-4343-a5b0-5e2cf00d7086`,
  `bad95ddd-9439-46c5-9815-d6c9c69e29b6`, `53dbc6e4-0f48-4f0b-8ec4-b684430eb0a3`,
  `9f66f369-97d3-44e9-985c-e92d4fd84753`, `307a539c-b5f8-4e87-aa36-625a77d32cad`,
  `c186973d-e763-4737-9b7a-f942756849bf`, `e43dd4e7-1936-46a9-b33f-7a1a15d40eb6`): 1826 passed, 73 skipped by the specs
  themselves on terminal hosts, 1 failed; one sandbox (`9f66f369`) ran none of its 16 projects, because the run's own
  setup check looked for the stack's first session about a tenth of a second before the stack listed it (a race in the
  host-terminal run's setup, not in the plans; one observation, its record kept, and not recorded as a flake; a setup
  that waits for the listing would be a follow-up). Those 16 projects then ran on their own (runs
  `65acb29f-54bc-4a84-9c51-5a082d956b68`, `2602f360-3d7a-4fc8-8962-f3392d1cdb2a`,
  `500e85ea-d64c-4ddf-bdd9-9a62d8d5830d`, `edc59ec3-4897-475b-bf0d-b55677948b97`): 196 passed, 2 skipped.
- Each failure was re-run three times on the same build, also on sandboxes (runs `b2312456-07aa-4134-9e92-927b9588021c`,
  `067263e3-ade5-4a32-a066-285f75d8a38d`, `d56a02bc-fb89-4fc5-a2de-cbd08bda9ce9`,
  `9e61f589-570c-46c8-99ca-e9cc59a2b1d9`, and for the host-terminal one `4d251630-f1a4-4b1e-9ad0-93c5e6da42f4`). "Trash
  header and global confirmation remain reachable at width 360" (Chromium; its cleanup failed) and "terminal link
  ownership opens once and preserves program clicks in all mounted terminals" (WebKit; a terminal did not show its
  agent's ready line in time, before any link was clicked, in terminal code whose drop and drag-hint handling
  grid-drop-and-drag-notice changes) passed 3 of 3; neither is recorded as a flake, and one failure each is not recorded
  as one. "The dialog refuses blank and over-long messages without sending" (Chromium) is the recorded open flake of the
  feedback dialog's Send color, here for the first time on Chromium and the other way round (the dark-theme pass kept
  the light background); it passed 3 of 3. discord-link-in-app adds a line to that dialog, but its only stylesheet
  change lays out the dialog's summary, not the Send button, and the failing check is the recorded one: the button's
  color right after the test switches the color scheme. The new evidence goes to the flake's record in a separate change
  after this round. "Post-clone commands with carriage returns refuse edits but can be cleared" (WebKit, host-terminal
  run) is recorded as a flake by an open draft PR (#2549); it passed 3 of 3. "Closes a number popup below a new bolt
  chord" (WebKit, template quick-launch numbers) failed once more in the three, both times as a 60-second test timeout
  waiting for the browser to report that an already-successful save request had finished. No plan of the round touches
  templates or quick launch. On main `422b081f` without the plans it passed 10 of 10 on WebKit and 5 of 5 on Chromium
  (runs `3286550d-b990-4fad-9ec6-ca57bc100270`, `a2ceae48-21e8-4a31-8235-f587d497501a`,
  `8da9cefb-0609-4e7f-91ba-6ae97284b89e`), and on this stack, rebuilt, 10 of 10 on each engine (runs
  `adfbbc0c-dfdd-4530-b01a-496e8a60005d`, `0247e4a0-7fe7-4eb8-be18-e74dcc932988`): two failures in fourteen WebKit runs
  on the stack against none in ten on main, which does not tie it to the plans. Its cause is not established; both
  failures came from one build within an hour and the same build passed twenty times after, so it is not recorded as a
  flake on these two observations; the failed runs' records are kept, and a later failure would make it one.
- Reused: faster-snapshot-encoder's release-mode emulator measurements (memory, the full round trip, encode speed), as
  its report lists them, on its own commits before the rebase: nothing that landed since touches the terminal emulator,
  so they still apply. Not run: the doctests (no doc examples changed), the desktop smoke tests (no desktop-specific
  code changed), the development screenshot review (only its help-menu count changed, which the feedback tests also
  assert) and the native Mac app (not runnable on Linux; drops from Finder remain a manual check).

The executor's report names its commits before the landing's rebase and calls the PRs drafts; what landed is those
commits rebased onto current main. Nothing else in it became untrue at landing.
