# Launcher permission choices and warning colors

## What this was about

Typing `yolo` before choosing an agent answered “Choose a harness first” in ordinary text, which looked like a broken
feature. The permission buttons already allowed that order. The requested change was to make launcher rejections
conspicuous in yellow, allow the same permission choices through search, and reserve red for danger.

The launcher now offers Guarded and YOLO before an agent, through their buttons, bare words and `perms:` words. If
choosing the first agent replaces an explicitly chosen permission with a more dangerous one, the existing red banner
appears and Undo restores the earlier choices, including having no agent selected.

## Things you should know

Yellow applies throughout the shared launcher, including New, Clone, Replace with and Restart with: search refusals,
request errors, directory and checkout errors, template failures, and a failed save of the YOLO “don't ask again”
answer. Bordered launch alerts keep their icon and border. Loading and ordinary typing hints stay plain. The permission
escalation banner, its outline pulse and YOLO danger markings stay red.

A merely displayed default or a remembered permission does not trigger a banner when choosing the first agent. Other
permission modes and effort still need an agent first. The Command tab offers no permission rows. Existing permission
reconciliation and Undo are reused.

The product and implementation specifications and the Start a session documentation were updated. The original TODO
entry was removed. The two PRs remain drafts for the landing monitor.

## Open questions and possible follow-ups

None required for this plan. There are no omitted acceptance criteria or unanswered design decisions.

## The PRs

- [#2987: yellow launcher rejections](https://github.com/scode/farhelm/pull/2987/changes).
- [#2989: permissions before choosing an agent](https://github.com/scode/farhelm/pull/2989/changes).

## Checks run, reused and skipped

Runtime checks used the test recorder, with pinned nextest for Rust and pinned tmux for browser stacks. Counts below are
case executions and overlap between runs.

- `cargo nextest run -p farhelm-ui -E 'test(launch_composer::) | test(create_form::)'`: PR 1 passed 167 selected cases,
  run `307eccd3-1097-44c7-8051-2d00bef46388`; the combined change passed 168, run
  `7ed04039-468f-46b9-9f71-eeaf6af1d05f`. The other 675 cases were filtered out in each run. No runtime substrate skips
  occurred.
- Selected `composer-word-search`, `yolo-guard` and `github-checkout-composer` browser cases passed 18/18 on Chromium
  and WebKit, run `9c329886-565c-4d90-8859-dedfad3eb794`. These cover both themes, ordinary hints, launch alerts, a
  failed YOLO preference save and a failed repository scan.
- The same applicable selection with `FARHELM_E2E_SUITE_TERMINALS=host` passed 16/16, run
  `aabcfe07-4c1f-4943-b0ae-35249b9a1556`. That suite's inventory excludes `yolo-guard`; the ordinary run supplies that
  coverage.
- `npx playwright test 'permission-switch\.spec\.ts|composer-word-search\.spec\.ts'` passed 38/38, 19 per engine, run
  `7cb15ede-b183-4719-a82a-0822ec20dc05`. It covers the combined change, including first-agent escalation, explicit
  versus remembered choices, Undo, existing agent/model/template switches and banner lifetime behavior. No browser
  failures, skips or global errors occurred. `permission-switch` is outside the host-terminal inventory; these cases
  exercise launcher choices rather than a terminal.
- `cargo build -p farhelm`, the web build with `dx build --package farhelm-ui --platform web --release`,
  `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt -p farhelm-ui -- --check`, configured Markdown
  formatting checks, `python3 releasing/check-changelog.py format`, and the isolated-interpreter
  `python -B scripts/check-test-sleeps.py` passed. The delay checker inspected 403 calls, with no missing rationale.

The final browser run covered `9dee4e209b809faa50ed213712ecc415e4d53a8b`. The stack was then rebased onto main's
unrelated TODO addition. Both PRs have an empty interdiff against their reviewed versions; executable sources and
selected tests are unchanged. Final heads are `efd884e24d6e679158688e703514bc2c06d9e7a5` and
`5ced6faa356ad643305f3f889aff910df8322c1d`, so the recorded checks remain applicable. The backend build is reused
because this plan changes no backend code.

Full workspace, full browser, desktop, installer and provisioning suites were skipped: the identified risks are launcher
search, permission transitions and presentation, covered above. The incidental docs edit changes prose without moving
pages or changing links, so no website build or preview was needed. CI is manual and was not dispatched.

## Review gate outcome

Each PR passed its required independent gpt-6.1-sol/high review. PR 1's reviewer found two missed warning paths, the
failed YOLO preference save and repository discovery failures; both were corrected and the review passed. PR 2 passed
without findings. Commit messages and PR text passed fresh-context cold reads. The rebase preserved both reviewed
patches without changes.

### Landing

Landed with no fixes at landing. The launcher's refusals and errors now show in yellow, keeping red for danger, and
Guarded and YOLO can be chosen before an agent, through their buttons, their bare words and `perms:` words in search.

- #2987: yellow for the launcher's refusals and errors.
- #2989: permission choices before an agent, the specification, a docs page and the removed TODO entry.

#### What landed since the stack was based, and whether it interacted

Nothing outside plan bookkeeping landed on main since the plan's base, and nothing else landed in this round. A
fresh-context review also read the plan against the plans landed shortly before its base (launch-progress, the in-app
announcements, colored agent marks, the Discord link) and found no interaction: the new yellow rules use the existing
warning tokens for both themes and win only inside the launcher (the shared red error styles stay red in Hosts, Settings
and elsewhere), launch-progress's styles are untouched, and the new choice depends on the permission chosen, never on
which agent, as the repository's per-agent rules require.

#### Things you should know

- On the Agent tab with no agent chosen, which is how a new session starts, the bare words match any part of `yolo` or
  `guarded`, as they already did once an agent was chosen: typing `y`, `yo`, `a`, `ar` or `guard` now shows a permission
  row, which hides the "Set session name" row, and `yo` then Enter picks YOLO rather than naming the session. The plan
  kept the existing matching rules, and the review found no browser test that types such a short search. Since `yo` then
  Enter is the risky direction, a follow-up could match only from the start of the word, or only from a minimum length,
  until an agent is chosen.
- One unit test still expects "Choose a harness first" for `yolo` and `guarded`, which the launcher can no longer show
  for them; it tests a path that is now unreachable, and dropping or rewriting it is a follow-up.

#### Checks

This round landed only this plan, stacked on main `1c6732f7`. On the stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 403 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older reports), the docs website build (`bun run build` in `website/`), the workspace debug build and
  the UI's release web build: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui` (all of them; the executor's selection had left out the
  permission-control tests), run `a07da40c-ad00-44c4-a3a9-b0463c561673`: all 843 passed; its desktop-build tests,
  `cargo nextest run -p farhelm-ui --features desktop`, run `ecc11fd8-6ab3-4e0b-862a-4c90b3934d64`: all 1032 passed. The
  UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `7014b925-d330-4bcf-87b3-267c8e57724f`: all
  516 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack; Node 24.16 there, against 24.20 locally), runs `3ddbdfb1-aad9-48eb-ac44-c90602de56c3`,
  `38f08100-ba8a-4799-a390-6883b2b0cd9c`, `569f1986-2f6f-4c04-9523-ab69ec27c3aa`,
  `55c4bee4-892d-4118-968c-d37cca4c920f`, `ecb2845b-15c7-4d2d-bef1-2db8b4b42936`,
  `8add8072-2ed9-4899-a9ac-d1f5bf3c95d3`, `2ec195f1-9a60-4239-84a2-ea236ce16848`,
  `b5d2d38a-e299-411f-8c86-d19be5f14701`, `dac559a7-3b6f-4790-8237-30a11711165a`,
  `3e903ba4-1227-44b5-8f91-1b501f79e9f9`: 2932 passed, 18 skipped by the specs' own design, 6 failed. Three are recorded
  flakes: "post-clone commands with carriage returns refuse edits but can be cleared" (WebKit, recorded by open draft PR
  #2549) and "trash header and global confirmation remain reachable" at both widths (Chromium, recorded on 2026-11-04).
  The other three re-ran three times on the same build, also on sandboxes. "Changed-host advice while following:
  identity" (Chromium; an update's progress label showed no step name for the whole 5-second wait) passed 3 of 3 (run
  `f30c513a-8a65-4a7a-b566-f0d745ec0f86`), a single failure not recorded as a flake. "Editing the working directory
  after a failed create mints a new intent key" (WebKit) passed 3 of 3 (run `81ce6bcb-838f-40b3-94c8-65c4f1b3d478`); its
  failure was the launcher's "Runs in YOLO mode" choice of No not taking, at the same step of the create form shared by
  several tests, and the same thing failed two multi-host tests in the two previous rounds, both before this plan
  existed, so it is not this plan's. "A pending post-clone save orders reopened settings before the next save" (WebKit)
  failed once more in the three (run `f4743ad3-5245-4951-bd1f-d78e374312dc`), as a 60-second timeout waiting for an
  already-answered save request to finish, as in the previous round; it is in settings code this plan does not touch.
  With three failures each across rounds, the radio step and this settings test are latent flakes; both are recorded in
  the flake log in a separate change after this round.
- Reused: the executor's run of the launcher's browser specs on terminal hosts (16 passed, on its own commits before the
  rebase, which changed nothing outside plan bookkeeping), in place of the browser suite's host-terminal run. Not run:
  the whole Rust workspace (only the UI crate changed, and only internally), the doctests (no doc examples changed), the
  desktop smoke tests (no desktop-specific code changed), the installer tests (unchanged), the development screenshot
  review (no shot changed) and the native Mac app (not runnable on Linux).

The executor's report names its commits before the landing's rebase and calls the PRs drafts; what landed is those
commits rebased onto current main. Nothing else in it became untrue at landing.
