## What this was about

New terminals refused dropped files and images, and gave no explanation when a program kept a mouse drag from selecting
text. The maintainer chose parity with old terminals, plus moving the existing hint away from the text just dragged
over. Both kinds now accept the same drops and show the same drag-copy guidance. The final PR removes both source TODO
entries.

## Things you should know

Files, images and text dropped onto a new agent terminal or shell tab use the existing upload and paste handling, with
the same payload precedence, directory refusal and disconnected-terminal refusal. The unsupported-drop switch and
message are removed.

After a plain drag goes to a program that uses the mouse and the program does not copy, new terminals show the existing
hint, including Codex's own instruction in its agent terminal. Wording, the 1.5-second program-copy grace, 30-second
lifetime and dismissal behavior are unchanged. A primary-selection write counts as the program copying even though the
browser does not put that selection on the system clipboard. Link drags withheld from the program and frozen takeover
screens never raise guidance.

In both kinds, the hint's bottom edge sits three terminal rows above the release point when it fits in the terminal and
window. Otherwise its top edge sits three rows below. A very short terminal keeps the previous placement clamped inside
the window, which can overlap the release row. Horizontal centering is unchanged. The gap follows the terminal's text
size.

The docs pages changed are `website/src/content/docs/docs/using/new-terminals.md` and
`website/src/content/docs/docs/using/work-in-a-session.mdx`. No preview was started during unattended execution. No
native drag handler, clipboard protocol, supervisor behavior or live installation changed.

## Open questions and possible follow-ups

The remaining manual check is a real Finder drag into the Mac app on new terminals: a file onto the agent terminal, a
file onto a shell tab, and a directory refusal. Browser tests synthesize drop data and do not establish native macOS
delivery. The existing desktop checklist now covers both terminal kinds.

The hint remains guidance, not a clipboard-success report. Existing clipboard limits can discard a program write before
the browser sees it, so such a write can still leave the hint visible. This plan changes neither those limits nor the
silent best-effort clipboard policy. No unresolved product decision blocks the stack.

## The PRs

- [#2965](https://github.com/scode/farhelm/pull/2965/changes): accept file drops in terminal hosts, head `2cc5471e`.
- [#2969](https://github.com/scode/farhelm/pull/2969/changes): show drag-copy guidance in terminal hosts and move it
  away from the release row, head `fcbae78f`.

Both are open drafts in one linear stack, one commit per PR. Neither was marked ready or merged by this executor.

## Checks run, reused and skipped

Runtime commands ran through `scripts/record-test-run.py` on Linux. Live browser runs used Chromium and WebKit, three
owned stacks, three worker slots, zero retries and pinned tmux 3.7c. Their complete reports passed policy checks,
retained untruncated output through EOF, and completed cleanup. Host-mode runs explicitly retained
`FARHELM_E2E_SUITE_TERMINALS=host` and confirmed the host-terminal premise. No live browser case was skipped.

- `cd e2e && npx playwright test 'terminal-attachments\.spec\.ts'`: 76 passed on old terminals, run
  `aab794bf-869a-4b2c-ac6b-2a86a53617eb`, and 76 on new terminals, run `b0246fd9-b33f-48c9-b319-6c8596ea6f29`. Each
  engine ran 38. This includes actual drop gestures on agents and tabs, precedence, bracketed paste, directories,
  interruptions and disconnected refusal. These PR1 results at `7bf4fcac` are reused for PR2 because upload, drop and
  paste behavior did not change; the added notice teardown is covered separately.
- `cd e2e && npx playwright test 'mouse-modes\.spec\.ts'`: six passed on old terminals, run
  `2712abb9-d360-4521-a4ba-cce5a13d988f`, and six on new terminals, run `a437c583-acc0-407c-bdec-d6cbcd981d15`, three
  per engine. This proves generic and Codex wording, program-copy suppression, click exclusion, placement, repeated
  display, dismissal and focus/input isolation, hidden-tab behavior and the actual 30-second lifetime.
- `cd e2e && npx playwright test --config grid-client/playwright.config.ts grid-attach.spec.ts --workers=2 --retries=0`:
  PR1 passed 68 cases, run `c7fab71d-d168-4d0b-88d1-690ebd6d953b`. PR2's expanded run
  `c95146a1-847b-44ae-9422-2169ac8588db` had 68 passes and four failures in two new fixtures, on both engines: a plain
  URL was incorrectly expected to show a hover label, and a frozen screen retaining mouse reporting was incorrectly
  expected to select without the forcing key. Both fixtures were corrected without changing product code. The exact two
  cases then passed in Chromium, run `00f9610a-a0cb-4b6f-8bc7-9436b29d09a2`, and all four passed on both engines, run
  `7cdecfc7-70a2-4126-882c-7ece68a326bf`, using `-g 'a withheld grid link drag|a dropped connection keeps'`. The other
  68 cases were unchanged. The failed run and traces remain retained, not represented as a full-suite pass or a latent
  flake. The static tests also prove primary-selection suppression without copying and frozen-screen silence.
- `cd crates/farhelm-ui/js-tests && node --test`: PR1 passed 508, run `8872efff-51d3-47e6-960d-1968672974bb`; PR2 passed
  512, run `0057cc40-ce6a-45df-8f95-b983d4a76291`. Exact row-gap, above/below/fallback, font scaling, viewport edges and
  notice lifecycle boundaries are covered. Later edits changed only static browser fixtures, comments and an unused
  import, preserving this unit coverage.
- `bun run build` in `website` passed after each docs change: 35 pages and all internal links valid. Selected Markdown
  `dprint check`, changelog format and whitespace checks passed. The isolated delay checker inspected 397 delays with
  zero missing rationales. The instruction checker had zero errors and three pre-existing stale-citation warnings. Asset
  JS, CSS and TypeScript have no configured dprint plugin; no formatter coverage is claimed for them.
- The CLI, fixture and Dioxus 0.7.10 web builds passed. Rust formatting passed for PR1's comment-only Rust edit. The
  initial missing Astro dependency and unrelated `dx` command were corrected before successful builds, with no source
  workaround or shared-tool replacement.

Main through `27531ca3` was inspected in full and the stack rebased onto it. Intervening changes were TODO and planning
bookkeeping only, with no executable or specification interaction. PR1's interdiff was empty; PR2's tested behavior was
preserved, so the command evidence above was reused without another runtime run. The final unused-import removal has no
execution effect.

Rust runtime, clippy, native desktop, installer, provisioning and the full browser battery were skipped: no Rust
behavior or native handling changed, and targeted tests cover the changed browser contracts. Native Finder delivery
remains the manual check above. No hosted CI was dispatched.

## Review gate outcome

Each PR received the required single fresh-context gpt-6.1-sol review at high effort. PR1 had no findings. PR2 found the
retained-mouse-mode fixture issue; the same reviewer confirmed its correction and the OSC-link fixture correction, with
no remaining findings. No scope expansion or weakened guarantee was required. Commit and PR wording passed separate
fresh-context cold reads; an invalid reader that opened the diff before its blind phase was replaced before use.

### Landing

Landed with one comment fix at landing, last of the four plans landed together in this round. New terminals now accept
dropped files, images and text like the old ones, and show the same drag-copy hint when a program keeps a mouse drag
from selecting text; in both kinds of terminal, the hint now sits three rows away from where the drag ended instead of
over the text just dragged (a very short terminal keeps the old placement, which can overlap that row).

- #2965: drops into new terminals.
- #2969: the drag-copy hint in new terminals and its new placement, and the removed TODO entries.

#### What the landing fixed, and why

A comment in the grid terminal component still said the drag-copy notice is not built for grid terminals, which #2969
made false; the plan's sweep of stale statements did not reach that file. The landing reworded it to say the notice is
driven around the component by the terminal script. No behavior changed; the web bundle was rebuilt with it before the
checks below.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the grid terminal's link fix (#2949, already in this plan's base), recovery of
unavailable terminal hosts and tabs (#2950, #2956), the orderly stop's checkpoint changes (#2952, #2954), and PRs that
only added TODO entries and plans; none touches drops, selection or the drag hint. A fresh-context review found no
interaction with the other plans of this round: its changes to the terminal test helpers and the list of specs the
host-terminal browser run covers are comments only, and every caller of the hint, which now requires a placement, passes
one. The rebase conflicted only in TODO.md and in implementation-notes sentences changed apart; both were resolved
keeping every side's changes, and a check confirmed each commit changes exactly the sentences it changed when tested.

#### Things you should know

- The remaining manual check is a real Finder drag into the Mac app on new terminals (a file onto the agent terminal, a
  file onto a shell tab, a refused directory); browser tests synthesize drops.
- The validation inventory's entry for the grid-client browser tests does not mention the new drag-hint cases.

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
commits rebased onto current main, with the landing's comment fix in #2969's. Nothing else in it became untrue at
landing.
