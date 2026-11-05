# Hosts needing attention above sessions

## What this was about

Setup progress and host problems were easy to miss while Hosts was closed. Updates had separate banner and card
reminders. The requested change puts hosts needing attention together above the sessions, while keeping the full
host-management panel available from Hosts.

## Things you should know

The list shows problems, ongoing setup/update/uninstall work, and available updates. A host appears once: ongoing work
takes precedence over a problem, and a problem over an update. Problems appear first, followed by work and updates.
Choosing a host opens Hosts at its menu button without changing the session filter. Working rows show the operation and
progress; their full step details remain in Hosts.

Update acts on one shown host; the list's Update all acts on exactly its shown Update rows and appears only when there
is more than one. The broader Update all inside Hosts keeps its existing scope. Optional update reminders can still be
dismissed for the current interface lifetime; required updates, problems and work cannot. Reloading, a new update
identity or a changed helm build can reveal optional reminders again.

The mockup's unreachable-since timestamp was omitted as the plan directs: Farhelm records no such time. The existing
retry wording is retained. Ordinary Connecting and suspended hosts, healthy hosts and usable newer hosts do not appear
unless they have an independent warning or ongoing work. The banner and card are removed. No schema, protocol,
persistent preference or host-operation policy changed.

The implementation keeps the existing mounted operation owners and warning acknowledgement. A review found that
scrolling a whole expanded host row could leave its focused menu offscreen; the handoff now scrolls the menu button
itself. A browser regression proves the tall, straddling-row premise and visible focus in both engines.

## Open questions and possible follow-ups

No blocking question remains. The existing images on the two changed host guides show Hosts, dialogs or the navigation
rail; none was confirmed stale from this change. A dedicated illustration of the attention list is still absent. No
documentation capture or preview was started during this unattended run. The Mac app itself was not driven; WebKit
browser coverage and desktop compilation are the available evidence.

## The PRs

- [#3044: show hosts needing attention above sessions](https://github.com/scode/farhelm/pull/3044/changes), open and
  draft.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-ui --lib -E 'test(/^hosts::/)'`: 92 passed, 769 selected out; recorder
  `61bb17c3-e346-4f2f-9e5a-3a15d1daf22d`, four slots, zero retries, no tmux required. This covers classification,
  warnings, host projections and update authority. Reused after the focus-only correction and added browser regression
  because those Rust contracts did not change.
- Chromium and WebKit, expanded-host navigation regression: two passed; recorder `21b718b4-ac1c-4989-b582-ceaf174d7302`,
  pinned tmux 3.7c, zero retries. Both engines exercised the oversized-row and pre-scroll geometry assertions.
- Chromium and WebKit, `host-updates.spec.ts`, `host-rail-warning.spec.ts`, `hosts-panel.spec.ts` and
  `provisioning.spec.ts`, excluding the already-passed expanded-row case: 224 passed, 112 per engine, with no skips or
  failures; recorder `53dfe9cf-fcbb-45f8-ac80-621012451f2f`, three owned stacks, zero retries, pinned tmux 3.7c. Four
  retained short-sidebar screenshots were inspected: both palettes in both engines keep the header actions and sessions
  usable while long host names truncate and the host rows scroll.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings` passed on the final PR revision, including the corrected
  focus handoff.
- `cargo check -p farhelm-ui --features desktop`, final web build, backend and fixture prerequisite builds,
  `cargo fmt --all -- --check`, supported changed-file Markdown formatting, TypeScript formatting, changelog-fragment
  format and the isolated test-sleep checker passed. The checker inspected 412 delays with zero missing rationales. CSS
  and MDX have no configured formatter here and were reviewed directly.
- The ordinary workspace and full browser batteries were skipped: the affected host flows and direct
  classifier/navigation checks cover the changed behavior. Backend runtime suites were unnecessary because no backend
  code changed. No website build was needed because the guide edits change no links, headings or frontmatter. CI is
  manual-only and was not dispatched. No live installation was changed.

Runtime evidence was collected on the plan's base plus its working changes. Subsequent main updates changed another
plan's tracking, unrelated quick-launch cleanup, and a startup check used only by the separate host-terminal browser
run. This plan's ordinary browser run does not enter that check. The careful rebase preserved both sides of shared
follow-up lists and has an empty interdiff for this PR, so its feature and relevant validation remain unchanged.
Complete recorder reports and output were checked; no runtime pass is inferred from a skipped substrate.

## Review gate outcome

A fresh gpt-6.1-sol/high reviewer found one navigation issue, which was corrected with the expanded-row regression. A
second fresh reviewer of that exact model and effort reviewed the corrected whole diff and reported no findings. The
commit/PR text passed its independent cold read. The documentation pass covered every changed code file and test.

### Landing

Landed with one fix at landing. Hosts that need attention (a problem, setup, update or uninstall in progress, or an
available update) now appear in a list above the sessions, replacing the separate update banner and card.

- #3044: the attention list, its styles, the rewritten host-updates browser spec, and the two hosts guides.

#### The fix made at landing

The progress bar on a row whose host has work running carried its "N of M steps finished" text in a native browser
`title`. Farhelm's controls show hover text through their own tooltip (`data-tooltip`), and the tooltip coverage test
refuses any native title; no test catches this one, because none shows a host with work running while it sweeps. The
landing changed that attribute to `data-tooltip` with the same text, in #3044's own commit, before the checks below ran.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained round 176's four plans. Round 176's listing change is in the supervisor; it adds
surviving tabs to a session's row but touches nothing the attention list shows, and none of the rest touches the hosts
UI. In this round it landed second, after terminal-text-drops, which changes only the vendored terminal code and the
terminal documentation. A fresh-context review found no interaction, and found that the other browser specs' shared host
helpers still work with the list in place.

#### Things you should know

- Every new button has hover text, so the list adds nothing to the tooltip coverage test's existing failure (the Add
  host form's "SSH" and "Tensorlake sandbox" choices, from #2973, which still fails on main). That test never shows the
  attention list, though, which is how the native title went unnoticed.
- SPEC.md, as this plan changes it, says a working row shows a progress bar when a step count is available. With no
  count, the code still shows an empty, static bar, on purpose, to keep the row's height fixed. Either the sentence or
  the bar should change; that is a small follow-up.
- That empty bar did not visibly move in Chromium or WebKit on Linux, so its reduced-motion rule has nothing to stop.
  The Mac app was not checked.
- The list and its scrolling body are two nested regions with the same accessible name, so a screen reader announces the
  name twice; naming only one of them would fix it.
- A host running a version too old for this helm, whose update offer has not loaded yet, briefly shows as a problem,
  then switches to an update row; a session menu open at that moment closes.

#### Checks

This round landed two plans, stacked in queue order on main `5065e022`: terminal-text-drops, then host-attention-list
with the landing's fix. The checks ran once, on the whole stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 412 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed.
- The vendored terminal crates' own tests, which the workspace run does not reach:
  `cargo test --manifest-path vendor/vte/Cargo.toml --features ansi`, run `82853607-026d-4030-830f-aec8413ba56e`: all 62
  passed; `cargo test --locked --lib --manifest-path vendor/alacritty_terminal/Cargo.toml`, run
  `931aa4d3-f93b-41dc-922e-d6d1aa568b27`: all 165 passed;
  `cargo test --locked --manifest-path vendor/pair-tests/Cargo.toml`, run `bb116fd9-9df3-4b0b-b567-f33693c4f92b`: all 3
  passed.
- The whole Rust workspace with the pinned tmux, `cargo nextest run --workspace --exclude farhelm-desktop`, which
  includes the terminal emulator's snapshot tests and its parity tests against tmux, run
  `3ae3f8ee-2143-4c6e-84bf-e5b9ac48362a`: 5608 passed, 70 skipped (ignored by design), 1 failed. The failure is
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, a recorded flake that sweeps already exclude.
  The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `a7bf68d7-e73f-402c-9ac1-a43bfb6ea92b`: all 862 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `5fc3d8d7-98f6-4625-b09b-b9d26d8fd812`: all 1051 passed. The UI's JavaScript tests, including the check that the
  stylesheet uses only design tokens (`node --test` in `crates/farhelm-ui/js-tests`), run
  `43f3b957-e103-4be3-a85e-2b589d7ffd6e`: all 516 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack), runs `1575baf5-5085-499e-a5a6-c00ff6ba569d`, `4ee46d8a-93af-4c23-8707-74304f887034`,
  `8438552a-ef03-4b89-aa29-333460b7f03c`, `7ce4c8ee-3e11-4c6e-b145-a8a1c3bf4995`,
  `bb8b35cb-8801-4859-a7a3-2170b2df5799`, `6d40c18f-2965-466d-98d4-6d6e0e71d877`,
  `bd11c081-e09d-4e81-b13f-107736cca84f`, `f97c8a59-f1f0-47a2-8f9a-40e55d13606e`,
  `a6827ea7-6fcd-40ad-aed7-ff5261ae37a8`, `164950b5-a9c0-497b-94e8-38cd1d789a0a`: 3006 passed, 18 skipped by the specs'
  own design, 2 failed. Then the same suite with every stack's terminals in terminal hosts, since host terminals render
  through the changed terminal emulator (`FARHELM_E2E_SUITE_TERMINALS=host`; this run covers only the specs listed for
  host terminals in `e2e/host-terminal-specs.ts`, so it holds fewer tests), runs `dd333943-4496-4b5d-86c5-f53574292f10`,
  `bf84db92-1302-4a47-a4a9-1e79f8322fe4`, `1a9080ad-2ab6-47c2-a1c6-0d0b74aa88ad`,
  `72f532ec-617f-43e9-97b5-f175663add10`, `ffd70aab-38f9-4fa4-a7e8-af8fce9bced0`,
  `cdc217a1-c48a-46af-846f-3ab6c4dbd8cf`, `155ec254-5415-4214-896d-ba2c3174d224`,
  `f5dc179b-e410-46b0-b8fb-cac944c6c12c`, `16ab6c47-8c70-4a37-ab2c-4b0cde0caeb9`,
  `1908e893-140c-4e6e-a665-0d18520bd29e`: 2082 passed, 75 skipped by the specs' own design, 3 failed. Of the five
  failures:
  - "Sidebar surfaces give every control hover text" failed four times, in both engines in both suites (runs `1575baf5`,
    `f97c8a59`, `72f532ec`, `f5dc179b`), each time naming only the Add host form's "SSH" and "Tensorlake sandbox"
    choices, which have no hover text. Those came with the Tensorlake sandbox hosts (#2973), on main before both plans'
    bases, and the two previous rounds saw the same failures. It is a break on main and a follow-up there; nothing in
    TODO.md or the review queue records it yet.
  - "Trash header and global confirmation remain reachable at width 360" (Chromium, run `72f532ec`), a recorded flake.
- Not run: the doctests (no doc examples changed), the grid component's browser suite (no grid code changed), the
  desktop smoke tests, the installer tests, the development screenshot review (no shot changed), the docs website
  preview (the build above passed; no page was viewed) and the native Mac app (not runnable on Linux).

The executor's report calls the PR a draft; what landed is its commit rebased onto current main with the one-attribute
fix above. Nothing else in it became untrue at landing.
