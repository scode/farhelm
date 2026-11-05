## What this was about

Choosing a host in the navigation rail gave little feedback, especially when no sessions matched. The session list now
shows the chosen host's mark and name beside its status choices, with a button to clear the host filter. An empty
filtered list names the host and status that produced it and offers **Show all hosts** and **Show all statuses** where
applicable. Clearing a filter returns keyboard focus to **All hosts** in the rail and keeps the open session.

The chip follows the current host choice. While another host's result is loading, the empty message keeps describing the
result still shown. Removed hosts remain named, with an enabled escape back to all hosts. Long names truncate at the
minimum sidebar width in the chip, while the empty sentence wraps the full name and the status choices and removal
control remain available.

## Things you should know

This is the agreed chip and message change. The requested broader review of session filtering remains as **Generally
rethink the session filtering experience** directly under Near term in TODO.md.

The existing development captures `visual-review/dark-wide-status-empty`, `light-wide-status-empty`,
`dark-main-650-status-empty` and `light-main-650-status-empty` have stale empty-message wording. Their assertion was
updated; captures were not refreshed. These development images appear on no docs page. The incidental docs update is
**Session list → Find a session**.

## Open questions and possible follow-ups

There is no decision needed for this change. The broader filtering work remains the requested follow-up above.

## The PRs

[Show the session list's active filters](https://github.com/scode/farhelm/pull/3054/changes) — one draft PR, based on
main.

## Checks run, reused and skipped

- Native listing and rail identity tests: **40 passed**, 1,045 selected out, no failures or runtime substrate skips.
  `CARGO_BUILD_JOBS=4 python3 scripts/record-test-run.py --runner nextest --kind development --tmux none --selection 'host filter rows and rail identity' --concurrency '4 nextest slots; retries 0; 4 cargo build jobs' -- cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(/^rows::tests::/) | test(/^hosts::tests::rail_identity/)'`.
  Pinned nextest 0.9.148 was prepared with the repository installer. Run `3dc407cb-5fda-4f6b-8150-f3e455b8d621`.
- Stylesheet contracts: **53 passed**, no skips, covering tokens, contrast and animations. From the UI JavaScript test
  directory,
  `python3 ../../../scripts/record-test-run.py --kind development --selection 'host filter stylesheet contracts' --concurrency 'node default threads' --tmux none -- node --test app-css-tokens.test.js app-css-contrast.test.js app-css-animations.test.js`.
  Final run `9f49ae2d-105d-4c6c-9841-f255f16ac286`, selection label `host filter feedback: final stylesheet layout`.
- Default browser checks: **42 passed**, 21 per engine, no skips or failures. From `e2e/`,
  `python3 ../scripts/record-test-run.py --runner playwright --kind development --tmux required --selection 'host filter feedback: filters and status, both engines' --concurrency '3 browser workers, one stack each; retries 0' -- npx playwright test 'tests/(filters|status-filter)\.spec\.ts'`.
  Run `6645b960-41d9-41d2-a2c7-7e1584c61a9b`.
- Final narrow-layout regression: **4 passed**, two alias forms in each engine, no skips or failures. The selection
  above became `tests/filters\.spec\.ts -g 'the host chip truncates'`, with selection label
  `host filter feedback: narrow spaced and unbroken names, both engines` and three configured workers (two used). Run
  `a2b55416-46c6-4dbe-9d3c-4f5b02db8a3d`. All eight actual dark/light captures were inspected: the full name and
  sentence-ending period fit, while the chip truncates without hiding status choices or its removal control.
- Host-terminal status parity: **14 passed**, seven per engine, no skips or failures. From `e2e/`,
  `FARHELM_E2E_SUITE_TERMINALS=host python3 ../scripts/record-test-run.py --kind development --tmux required --keep-farhelm-env FARHELM_E2E_SUITE_TERMINALS --selection 'host filter feedback: host-terminal status parity' --concurrency '3 browser workers, one stack each; retries 0' -- npx playwright test --project=chromium-status-filter --project=webkit-status-filter --retries=0`.
  Run `ca10928c-14dc-4ce7-b3c8-4b2d304273aa`, generic recording; counts come from the completed test output, with the
  host-terminal premise active. Two workers actually ran under the configured three-worker cap.
- Desktop compilation and linting:
  `CARGO_BUILD_JOBS=4 cargo clippy -p farhelm-ui --all-targets --features desktop -- -D warnings` passed.
  `CARGO_BUILD_JOBS=4 cargo build` passed for the current application and browser fixtures. The actual
  `dx build --package farhelm-ui --platform web --release` passed again after the final CSS correction.
- `cargo fmt --all -- --check`, `dprint check` on all changed Markdown and browser specs, fragment format validation and
  the isolated-interpreter test-delay check passed; the latter inspected 412 delays with none lacking a rationale. The
  agent-instruction checker passed with three pre-existing reference warnings. The website build passed for 36 pages
  with all internal links valid.

All runtime records identify base `309621f2a2b7f0cf1634ecf467bc11cce6682d6a` plus the working change. Native and the
initial browser runs have fingerprint `18d4723b6f1a913906ea3df35969a58efe4ddb879223aa42c57760cb358bc48b`; final CSS and
narrow-layout runs have `d469ab93c449f215b319d7be2f82e9b37631e29fecb59b41219d20e3853b1623`. Source and output are
complete, with no forced cleanup; controlled browser reports and policy checks are complete too.

The first narrow captures exposed clipped empty-message names despite passing assertions. The correction only changes
that sentence's wrapping and strengthens its layout test. The final four cases replace the earlier two narrow cases; the
other 40 default browser cases, 14 host-terminal cases, native tests and desktop lint are reused because their behavior
and Rust source are unchanged. Documentation/build checks remain applicable to unchanged inputs. A fresh fetch found
main unchanged, so no rebase or additional runtime checks were needed.

Full workspace, installer, provisioning, release, CI and native Mac runtime checks were skipped because the change is
confined to list presentation and the existing filter controls. The exact-text development visual-review case was not
run because it always captures images, which this plan explicitly leaves for a later refresh.

## Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, idiomatic code and the
changed tests. Its final review caught clipping of an unbroken host alias. That finding was fixed with message-only
wrapping and a glyph-containment regression; the reviewer verified the correction with no further findings. A fresh
scope review found no unnecessary mechanism or reason to expand the plan. Source review is separate from the runtime and
visual checks above. The commit and PR wording passed its fresh cold read.

### Landing

Landed with no fixes at landing. Choosing a host in the navigation rail now shows that host's mark and name beside the
session list's status choices, with a button to clear it, and an empty filtered list says which host and status produced
it and offers Show all hosts and Show all statuses.

- #3054: the host chip, the empty-filter message, the browser cases, the specification, the Session list guide, the
  changelog entry and the TODO entry it addresses (the broader filtering rethink stays in Near term).

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping landed on main since the plan's base, and nothing else landed in this round. A
fresh-context review read it against the plans that landed shortly before its base, which change the same session list:

- Marking several sessions: clearing the host filter changes only the filter, marks on rows the filter hides survive as
  the specification says, and the chip sits with the status choices, apart from the heading the marked-sessions bar
  replaces.
- Pin order: the only change to the row ordering code is the empty message; removing the chip releases the list's
  temporary hold on its order (while the pointer is over it) the way any other filter change does.
- The host attention list: its rows open Hosts without touching the filter, and the chip and message hide while Hosts is
  open.

#### Things you should know

- One new spacing rule has no effect, because a later rule for the same element wins: the Show all buttons sit about 28
  pixels below the empty message instead of 8. Making the rule more specific is a one-line follow-up; the browser cases
  that check the message's layout would need running again with it.
- Besides the four empty-message captures the executor's report names as stale, other development-review captures taken
  with a host chosen (such as the rail-warning ones) now show the chip too. None is published.
- Two small gaps in the new browser cases: the long-alias case does not first check that the remote host has no
  sessions, as its siblings do, and the removed-host case's final check that a read without a host filter happened can
  be satisfied by the read at page load.
- The browser test of hover text failed in this round as in earlier ones, because the plain Add host form's "SSH" and
  "Tensorlake sandbox" choices have no hover text (from #2973). Nothing in TODO.md or the review queue records it yet.
  This plan's new controls have hover text.

#### Checks

This round landed only this plan, stacked on main `536a9944`. On the stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 412 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `fc437b94-d9fb-4007-a5cd-171187250476`:
  all 527 passed. The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `1b586a29-a1b4-4981-a81a-0c577e606e22`:
  all 896 passed; its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `dc02341c-37fc-4713-bad7-3f5f976ff375`: all 1085 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack), runs `74cab069-c2d5-4d00-8b7a-b22f3add9a67`, `2794b6d3-9b40-4f62-a1d6-056768fe7fa6`,
  `f59165ac-1732-47ce-9312-11b4ed62d6fd`, `541a7a0e-0619-4e57-b9ba-2d345c1279f8`,
  `24a92a6a-14a0-4b8e-8bad-24f81d17e0c9`, `666a45fe-fe6a-4289-adb0-313c16508bf7`,
  `99f322a8-dd6c-46ef-93ae-11489269cb88`, `6609cccf-8cdd-424e-bc84-087db6e3c53b`,
  `ed60b475-9512-4a67-94f9-eb7a0d028ada`, `0c6da584-4dd5-4fd5-ad85-087f2004cddd`: 3083 passed, 18 skipped by the specs'
  own design, 3 failed. Then the same suite with every stack's terminals in terminal hosts (this run covers only the
  specs listed in `e2e/host-terminal-specs.ts`, so it holds fewer tests), runs `0bb0e098-6207-46db-b77d-f2d90998fc10`,
  `b3e6fe0f-83dd-4bbc-bc8e-a7f9c38e589a`, `365d96f9-062e-4e2f-bc24-5c8359876f54`,
  `ef35f0a8-eaeb-4258-af13-dbfc2d30a1ae`, `4b6e1aad-91ab-4aee-b721-f760fb42b0cf`,
  `d8b66c3f-127a-4816-9ee2-85fe564e2385`, `914f79ca-db11-4560-a9ae-bd0a1240447d`,
  `0ba415c9-25bf-4f3b-8972-31548cedbaea`, `d04bb06c-c518-413d-ab84-f1af5d2ae4d9`,
  `d7039a68-b26d-44ae-badb-76f412a3e7ca`: 2111 passed, 75 skipped by the specs' own design, 4 failed. Of the seven
  failures:
  - "Sidebar surfaces give every control hover text" failed four times, in both engines in both suites (runs `99f322a8`,
    `6609cccf`, `0bb0e098`, `0ba415c9`), on the Add host form's "SSH" and "Tensorlake sandbox" choices (see Things you
    should know).
  - "Trash header and global confirmation remain reachable at width 360" (Chromium, run `b3e6fe0f`), a recorded flake.
  - "A first identity learned while following: completion" (WebKit, run `541a7a0e`): an update's progress label showed
    no step name within five seconds. It passed three re-runs on the same build, on sandboxes, run
    `26350798-45c1-4214-9580-a064ea065d53`. The same assertion failed in two related provisioning tests in earlier
    rounds, before this plan existed; the landing monitor is recording it as a latent flake in FLAKES.md after this
    landing.
  - "Typed page refusals are atomic and empty Enter leaves later bindings paired" (Chromium, in the host-terminal suite,
    run `4b6e1aad`): focus did not return to the page body. It failed once more in three ordinary re-runs (run
    `443ebd09-f5c5-4cd3-8ad1-d27e96a75532`) and passed three host-terminal re-runs (run
    `65431942-c7a7-4344-9529-55d0d7a2b493`). The same test failed the same way in an earlier round, before this plan
    existed, and this plan does not touch quick launch; the landing monitor is recording it as a latent flake in
    FLAKES.md after this landing.
- Reused: none. Not run: the whole Rust workspace (only the UI crate changed), the doctests (no doc examples changed),
  the grid component's browser suite (no grid code changed), the desktop smoke tests and the installer tests (no desktop
  shell, packaging or installer code changed), the development screenshot review (it refreshes captures, which the plan
  left for later; see Things you should know), the docs website preview (the build above passed; no page was viewed) and
  the native Mac app (not runnable on Linux).

The executor's report calls the PR a draft; what landed is its commit, unchanged, on current main. Nothing else in it
became untrue at landing.
