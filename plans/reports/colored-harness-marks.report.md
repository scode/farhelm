## What this was about

The session list showed every agent mark in the same grey, making agents harder to identify at a glance. The maintainer
chose distinct colors where the owner's terms allow, preferring official hues and keeping the shapes unchanged. Seven
agent marks now have identity colors in both themes. Codex, Goose and the ordinary Terminal mark keep their existing
grey; every selected mark turns white.

## Things you should know

The source and terms re-check was made on 2026-11-03. These are the final dark and light colors:

| Agent    | Dark / light      | Source                                                                         |
| -------- | ----------------- | ------------------------------------------------------------------------------ |
| Claude   | #d97757 / #d25e39 | Terracotta from Anthropic's official palette, on Farhelm's original starburst. |
| Muse     | #456bff / #456bff | Farhelm blue; the community mark's source blue is unofficial.                  |
| Cursor   | #2fbfaf / #249084 | Farhelm teal; the checked official artwork is neutral.                         |
| Pi       | #4d9abf / #3e89ad | Blue from the press kit's primary logo.                                        |
| OMP      | #9b4dff / #9b4dff | The middle purple stop of the official favicon's gradient.                     |
| Grok     | #d8b839 / #987f1e | Farhelm gold on its original G; inspected official artwork is neutral.         |
| OpenCode | #83b94b / #638d37 | Farhelm yellow-green; official artwork is white and grey.                      |

The full source links and terms basis are in [the provenance record](../../docs/harness-marks.md). Pi's current press
kit now has a three-color primary logo, unlike the neutral favicon recorded during planning. The existing badge takes
one published hue, blue, under the plan's official-color fallback. It does not reproduce the primary logo's full
arrangement. OMP purple separates from that blue better than its cyan stop. Light-theme shades lower lightness where
necessary for contrast; the official hues are preserved.

The re-check also found current Grok brand guidelines and corrected the old claim that no kit existed. The kit ZIP
returned HTTP 403, so the color decision uses the direct favicon and official repository artwork. Farhelm's existing G
stays unchanged. Stencil's current OMP terms have broader scope than the old notes implied; the record now distinguishes
the MIT source and official monochrome implementation from an express trademark license. Cursor's referential-use
judgment remains the recorded basis; no express recoloring grant was found. No newly found prohibition required dropping
one of the seven colors. OpenAI and LF Projects restrictions still keep Codex and Goose grey, without claiming that grey
is an exact supplied black or white.

All seven colors clear 3:1 on both sidebar and hover surfaces in both themes; the lowest ratio is 3.113:1. Every pair
also meets the plan's CIEDE2000 separation target of 15: the closest is Muse/Pi in light mode, 15.995. The complete pair
table is in the PR description and working log. These measurements do not guarantee that every user can distinguish the
marks at 12px. Gold and yellow-green sit near permission-color families, and Claude terracotta is near warm host/status
colors; shapes and placement still distinguish those roles. Agent color means identity, never activity or permission.

I inspected both themes in Chromium and WebKit with all ten marks visible at their unchanged sidebar size. The seven
hues remain readable, the neutral Goose and Terminal marks remain grey, and selected Codex is white. Muse blue and Pi
blue are the closest pair, but their hue and shape remain distinguishable in these captures. The synthetic rows have no
real terminal behind them, so the capture's terminal-unavailable notice is expected; this checks row presentation, not
terminal attachment. No palette or geometry adjustment was needed after inspection.

## Open questions and possible follow-ups

No unresolved product decision remains. The docs website's session-list screenshots still show the old grey marks until
the next docs screenshot refresh. That published-media refresh was deliberately left outside this plan. The broader
distinction between an unchanged official shape and Farhelm's inherited grey treatment for Codex and Goose remains as
agreed; this change makes no new exact-color compliance claim.

## The PRs

- [#2961: color session-list agent marks](https://github.com/scode/farhelm/pull/2961/changes), draft at
  `a67d4bece15df0a7cfdc0a5acc5c6e8f1e0bb652`.

## Checks run, reused and skipped

Run on Linux:

- The final full asset JavaScript harness passed 508 tests, with no failures or skips: recorder
  `b417be3e-7bc6-4deb-9c23-8e4b40d6d9b1`. Child command: `cd crates/farhelm-ui/js-tests && node --test`. This covers
  token declarations, role routing, inherited neutral marks and the 3:1 floor against both surfaces in both themes. An
  earlier run passed 502 before main added six independent grid-link tests: recorder
  `f99a9ef6-8692-4066-903e-beab7690130b`.
- The focused ordinary browser run passed 10 tests, five per engine, with no failures, skips or flaky outcomes: recorder
  `1f45cee6-e9f6-427d-a3f2-58de47c9122a`. The final test source also passed the same 10 cases with terminal hosts:
  recorder `fdcd2f49-e3a6-4f49-845c-0a2f48955dc4`. Both used Chromium and WebKit, three owned stacks, three workers,
  zero retries and pinned tmux 3.7c. Both recorder reports are complete. The host run explicitly confirmed its terminal
  mode; neither output has a runtime `SKIPPED` marker.
- Browser coverage includes ordinary row metadata, compact marks, sidebar alignment, and the new dark/light cases that
  check every ordinary mark and actually select every row before checking white. The child command was
  `npx playwright test '(terminal|sidebar)\.spec\.ts' --grep 'agent marks keep identity colors|compact rows retain distinct ended and harness glyphs|list renders the session row with title|narrow rows align fixed facts'`.
  The host run retained the intentional `FARHELM_E2E_SUITE_TERMINALS=host` input through the recorder.
- `cargo check -p farhelm-ui -p farhelm-supervisor`, `cargo build` and the Dioxus 0.7.10 release web build passed.
  Markdown dprint, Rust formatting, changelog format and whitespace checks passed. The source delay checker inspected
  397 delays with no missing rationale. The instruction-anchor checker had zero errors and three existing stale-citation
  warnings outside this change.

The final JavaScript run covered the source tree committed as `a67d4bec`. Both browser runs and compilation covered the
same agent-color behavior on base `be255bc2`. The ordinary browser run preceded a capture-only refinement: the
screenshot now selects neutral Codex so all seven colored marks remain visible. Assertions, fixtures and product
behavior were unchanged; the host run executed the final version and supplied the inspected captures. Subsequent main
changes were inspected in full: grid-link joining and its tests, distinct specification sections, and TODO/plan
bookkeeping. They preserve the row rendering, CSS and selection contracts these checks exercise, so browser and compile
evidence was reused without another build or runtime run. Source checks were also reused because their covered edits and
formatting were preserved. No earlier-session runtime evidence was reused, and no runtime test failed during this plan.

The full Rust runtime, desktop native, website and unrelated browser suites were skipped because the behavior changes
only session-list CSS. The targeted browser checks exercise the actual cascade and selected-row behavior; compilation
covers the Rust comment edits and web bundle. Published screenshot capture and publication were outside the plan.

## Review gate outcome

The required single fresh-context gpt-6.1-sol high adversarial review found no actionable issues. It independently
checked the source records, recomputed all 28 contrast ratios and 42 pairwise distances, and inspected the CSS cascade,
unchanged shapes and markup, and test premises against the plan. Browser results were pending during that review and are
reported separately above. Commit and PR wording passed a separate fresh-context cold read.

### Landing

Landed with no fixes of its own, second of the four plans landed together in this round. Seven agent marks in the
session list now have identity colors in both themes; Codex, Goose and the Terminal mark stay grey, and a selected row's
mark turns white.

- #2961: the colors, their contrast tests, the provenance and notices, and the specification text.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the grid terminal's link fix (#2949), recovery of unavailable terminal hosts and tabs
(#2950, #2956), the orderly stop's checkpoint changes (#2952, #2954), and PRs that only added TODO entries and plans;
none touches the session list or the stylesheet. In this round, discord-link-in-app also edits the app's stylesheet; a
fresh-context review read both against each other and found no overlap: the selected-row rule still outranks the new
agent colors, so selected marks stay white, and the colored mark appears only in the session list. Each agent's color is
set in one place, which the code's index of per-agent differences now lists, as the repository's rules for per-agent
behavior require. The rebase conflicted only in TODO.md and in implementation-notes sentences this plan and others
changed apart; both were resolved keeping every side's changes, and a check confirmed the commit changes exactly the
sentences it changed when tested.

#### Things you should know

- A future agent's mark falls back to grey; no test forces a color decision for a new agent.
- The documentation screenshots still show the grey marks until their next refresh.

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
