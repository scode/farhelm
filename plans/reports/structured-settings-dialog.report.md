# Settings sections

## What this was about

Settings had become a long scrolling page. Related choices were hard to find, and reaching the trust controls meant
scrolling past unrelated settings. The agreed design was a section list like Templates, with drafts preserved when
switching sections and a one-section-at-a-time layout on phones.

The dialog now groups choices into sessions, hosts, terminal, checkouts, agent trust and notifications. Desktop apps
with an active updater also show app. The gear always opens sessions; trust hints open agent trust at the relevant
switch and its explanation. The setting labels, help, save behavior and storage scopes remain the same.

## Things you should know

The dialog is at most 760px wide, with a 148px section list. Header and footer stay pinned around one scrolling body. At
620px and below, it starts on the chosen section; **‹ settings** returns to the list. Section entries are ordinary
buttons using the platform's normal keyboard behavior. Selection focuses the new section heading, and returning to the
phone list focuses its selected entry. Escape and both close controls still return focus to the gear.

Switching sections keeps the existing controls mounted. Unsaved checkout and notification fields, save outcomes and
pending saves therefore survive navigation. Closing discards unsaved drafts as before; an explicit save already in
flight keeps its existing completion behavior. Device sound controls initialize even while their section is hidden. No
remembered-section preference, draft store, generic navigation component or new save mechanism was introduced.

The 14 affected guide pages, including the Manage hosts tour, describe the new locations. Its screenshot now shows hosts
beside the section list. All five locally emitted images were inspected, and the capture reviewer accepted the final
annotation placement. Nothing was published: the production image remains the preceding layout until the next full
screenshot refresh on latest main after landing. README, Overview, release images and screenshot metadata were left
alone. The completed TODO entry is removed in the final PR.

Main advanced during execution. The full intervening changes were inspected before rebasing onto bc6f5799. Shared dialog
framing and deferred host-dialog admission were the relevant interactions; the final browser selection covers those
interactions. Host-save refocus belongs to the separate host dialog and cannot take focus from Settings. The rebase
preserved upstream launcher changes and all completed TODO removals.

## Open questions and possible follow-ups

No product decision remains open. The next full docs screenshot refresh must publish the updated image after landing.
Native desktop controls and updater gating were checked in source; this run provides browser-engine coverage, not a
native desktop runtime verdict.

## The PRs

- [#2447](https://github.com/scode/farhelm/pull/2447/changes) — section navigation, preserved controls, affected browser
  coverage, specifications and changelog. Draft head `552d86af4c683dec4a85f37f24bff071b77820ac`, based on main.
- [#2448](https://github.com/scode/farhelm/pull/2448/changes) — updated guide locations and local capture specification,
  with the completed TODO entry removed. Draft head `6c3ffb85b8fd88d6af3afaf31832bb7415beedb9`, based on #2447.

## Checks run, reused and skipped

All runtime attempts used the test recorder. Rust used nextest 0.9.148, four slots and zero retries; browser checks used
Playwright 1.64.0, Chromium and WebKit, one worker and zero retries. Tmux 3.7c was required and verified. Successful
evidence has complete source identity, stream EOF, no retained-output truncation and no forced-cleanup limitation. No
runtime SKIPPED marker was found in the inspected output.

- `cargo nextest run -p farhelm-ui --lib`: 555 passed in run `77d5c3a6-3135-4901-8dad-c12e24f3fb30`, source 250edb69
  plus fingerprint `22fed872a33e243b76b2ac96a377b1dd652e40a1298ed39ab09bfb804a3e28bb`. These passes are reused for the
  unchanged Settings behavior; later Rust changes were formatting and restoration of two existing comments. Newly landed
  upstream unit cases are not claimed to have run here.
- Core browser command:
  `npx playwright test 'settings\.spec\.ts' 'claude-trust\.spec\.ts' 'workspace-trust\.spec\.ts' 'sounds\.spec\.ts' 'ntfy-settings\.spec\.ts' 'tooltip-coverage\.spec\.ts'`.
  Run `44ac4aa5-4287-4125-b823-c780dbb52201` executed 70 cases: 69 passed and one WebKit case failed before opening
  Settings, while waiting for the initial host row. Its trace showed host/session requests still loading at that wait;
  this is one startup observation, not an established latent flake or proven product regression. The failed run and
  trace were retained. Source 250edb69, fingerprint `185cbc35cbe18b721eaf9803a1f45c734e7355237f5a22676f771f053af939b4`.
- The exact unchanged failing case was then selected with the unique suffix
  `-g 'host browser permissions persist independently through the settings switches$'` in settings.spec.ts. Run
  `f6653b00-3321-43fa-a3b7-5aef9b04e8cc` passed both engine cases without increasing a timeout or changing product code;
  source 250edb69, fingerprint `4a360da9ea2f03f6a8b5e431b92b00bc6a95a0e0527b6e746faa615dcdb53894`. An earlier retry,
  `a629bebb-f255-4451-9b88-c8090abebfdb`, matched no tests because its grep anchored the short title at the beginning;
  it remains a retained selection failure, never a pass.
- Final browser run `31d47cb0-a5e7-4517-9ffe-4c72a0dd3b54` passed all 44 cases, 22 per engine, on rebased source
  bc6f5799 plus fingerprint `f51a909eabf15673b2d084bfb663f7c71e4a11028b2aff2759c514f5feb70dff`. The command was
  `npx playwright test` selecting provisioning, terminal-link-clicks, session-confirmations, quick-launch, terminal,
  session-action-shortcuts, settings and tooltip-coverage specs, with a grep limited to the moved-control, delayed-host
  dialog, modal shortcut, pinned-close, navigation and tooltip cases. It covers the 30 outstanding affected cases plus
  14 rebased frame/modal/navigation cases. All cases executed; none failed, skipped, timed out or remained unstarted.
  The earlier 69 core passes and two exact retry passes are reused for their unchanged test paths. The new navigation
  sizing assertion and shared-frame/modal interactions were executed again in this final selection.
- `scripts/docs-screenshots.sh --only manage-hosts --no-build` reused the matching rebased app builds. Final run
  `f9f0db09-095e-41d7-82c6-9d22dc788d9a` passed all 6 staging/capture steps and emitted five inspected PNGs. Source
  bc6f5799, fingerprint `f51a909eabf15673b2d084bfb663f7c71e4a11028b2aff2759c514f5feb70dff`. The preceding run
  `02853113-cae3-4b9e-9da4-16260f26255b` also passed its 6 steps, but its settings image failed visual inspection
  because an arrow crossed help text. That image and run were retained; they are not the final visual pass.
- Rebased `cargo build -p farhelm`, dx 0.7.10 release web build, `cargo clippy --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, scoped dprint and `python3 releasing/check-changelog.py format` passed. The isolated
  test-sleep checker passed 305 delays with zero unannotated. Frozen Bun install and the website build passed, including
  internal links. These cover the final implementation; subsequent capture-only annotation offsets do not alter the app
  builds, Rust checks, website links or delay inventory.

The full Rust/browser batteries and CI were skipped because the selected checks cover the changed controls and their
concrete interactions. JavaScript units were skipped because this plan changes no JavaScript. Native desktop execution,
installer, provisioning-distribution and release gates were skipped because their mechanisms are unchanged. No live
installation was modified.

## The review gate's outcome

Each PR received the required single fresh gpt-6.1-sol/high review for correctness, design and idiomatic code. The
structure reviewer checked the rebase, navigation sizing and test assertions, and accepted restoration of comments that
a formatter had misplaced. The docs reviewer found that side callouts would exceed the screenshot's fixed crop; moving
them above the dialog and inspecting the final images resolved that finding. No findings remain. Both message cold reads
passed. Separate documentation passes covered all touched files.

### Landing

Landed as #2447 and #2448, merged bottom-up. It was the first of four plans in this landing round, which stacked every
plan in queue order and checked them together before any merged. Its content is what the report describes, rebased onto
this round's main, with one SPEC.md conflict resolution, two SPEC.md wording corrections and one browser test fix,
described below.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained round 109's six plans, the browser suite's move to three stacks run in parallel, a
TLS certificate fix, documentation and plan bookkeeping. A fresh-context reviewer and I read them and the other three
plans of this round against this plan.

- select-newest-on-close, from round 109, added a clause to the same SPEC.md settings paragraph this plan rewrote:
  automatic session selection rereads the shared visits immediately before choosing. The two conflicted textually; the
  landing kept this plan's sectioned-dialog text and added that clause in its place.
- select-newest-on-close also makes a page save the automatic session selection while it loads. One older Settings test
  compared every preference before and after its interactions, so that save broke it; see the fix below. None of this
  plan's own new or changed tests depends on which session opens or counts preference reads.
- menu-arrow-focus-race, from round 109, added a focus check to menus. Settings does not use the menu components, and
  the check's page-wide focus listener does not interfere with focusing a section heading.

The other conflicts were in TODO.md, where round 109 had already removed neighboring entries and the resolution removes
only this plan's own entry, and in the provisioning browser tests' imports, where main's parallel-suite change and this
plan each added one import; both are kept. None of this plan's tests uses the parts of the browser-test harness that
change made per stack.

#### Fixes made during the landing

- SPEC.md: two leftovers from the old layout. A sentence about link clicks still named the old **terminal links**
  section, now **terminal**, and the sessions sentence said "under **sessions**" twice.
- The Settings test that checks empty space and container Enter leave a choice alone compared all preferences before and
  after; after round 109 the page load itself saves the selection, so it failed on Chromium on this round's stack (run
  `87ee3a27-84d7-4a4e-ab9f-8d478f5f4b35`, kept). It now leaves the selection fields out of that comparison and still
  proves the Settings interactions write nothing; it then passed three times on each engine (run
  `489cb959-129e-42d2-acd9-9f4e34350f0f`). The break comes from round 109's change, not this plan; the fix sits here
  because this plan already edits that test file.

#### Possible follow-ups

- Not from this plan, but found while landing this round: the sidebar test "the sidebar app bar explains the helm and
  window builds" fails on main two runs in five (see Checks). It fakes the helm's build stamp by intercepting every API
  reply, and a faked reply sometimes arrives after its request was already settled. It first failed on round 109's
  landed code, which added preference reads and a save while a page loads; that those requests are the ones racing the
  test is my guess, not established. Fixing the test, or recording it as a flake, is open.
- As the report says, the docs screenshot of the Manage hosts page still shows the old layout until the next full docs
  screenshot refresh publishes the image captured for this layout.

#### Checks

Every plan in this round was executed before round 109's six plans landed, so none had been tested against them, and
while this round ran, main also moved the browser suite onto three stacks run in parallel (each test worker runs its own
helm and supervisors). The plans were stacked again onto that main before the checks that count, so the browser evidence
below comes from the harness they land on. Together the four change how the supervisor runs every request, which helm
actions finish after a client disconnects, the Settings dialog and the launcher's Enter key; that reach is why this
round ran the whole Rust workspace and the full browser suite. All of the following ran on all four plans stacked
together on that main, including the fixes made while landing them:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the isolated
  test-sleep check, `dprint check` on every changed Markdown, JSON and TOML file,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `scripts/check-desktop-assets.sh`, and the website build (`bun run build` in `website/`): all passed.
- `cargo nextest run --workspace --exclude farhelm-desktop` through the test recorder with pinned nextest and tmux, four
  parallel test slots and no retries, run `fa6052db-ebdf-4204-8d20-25b8780367ec` (kept): 3977 passed, 1 failed:
  `terminal_backpressure::shallow_pause_resumes_without_reset_or_replay`, which has an open TODO deflake entry ("Shallow
  pause resets under load"). Because supervisor-request-modes changes how the supervisor reads messages from a host
  connection, I ran that test alone ten times on main without this round and ten times on this round's stack, minutes
  apart on the same machine: 9 and 10 failures. Both sides fail almost every time, so that comparison cannot show
  whether the plan changes the test; it does show the test is now close to always failing on main on this machine, where
  round 109 saw it fail one time in five. See the possible follow-up in supervisor-request-modes' notes.
- `cargo nextest run -p farhelm-ui --features desktop`, run `d8ff6545-9b27-45f8-aa33-7dfac8f07108` (kept): 724 passed, 1
  failed: a timing assertion in the tests of how the UI orders preference saves, code no plan here touches. Ten
  repetitions alone all passed, so I judge it machine load during the full run; on one observation it is not recorded as
  a flake. `node --test` in `crates/farhelm-ui/js-tests`, run `b88528dc-6d84-4d63-996d-56eb15c69510`: 328 passed.
- The full browser suite (`npx playwright test` in `e2e/`) on Chromium and WebKit, three workers, no retries, run
  `ac3bef42-b713-4f70-8e6e-0fd7b86e0a16` (kept): 2227 passed, 14 skipped by design (WebKit clipboard cases and tests
  that need a real signed-in agent or run on one engine only), 3 failed, all on WebKit:
  - a terminal test and a two-host test each saw a session marked as having new output where they expected a plain
    status. Neither test file is changed by this round, and three repeats of each on both engines then passed (run
    `8158eb60-714f-4b18-bf7f-51c8654840b8`), so I judge both timing under load with three stacks running; on one
    observation neither is recorded as a flake;
  - "the sidebar app bar explains the helm and window builds", which fails inside the browser tests' own way of faking
    the helm's build stamp: a faked reply arrives after its request was already settled. It fails on main without this
    round too, two runs in five (run `db0c82af-6a1e-4359-8724-7fa17c65dbba`), and first showed up on round 109's landed
    code, so it is not from this round; see the possible follow-up in structured-settings-dialog's notes.
- Before the parallel suite landed, the plans also ran on the earlier main. A narrow run there (run
  `87ee3a27-84d7-4a4e-ab9f-8d478f5f4b35`) found the Settings test failure fixed in structured-settings-dialog, and a
  full browser pass, stopped part-way when the parallel change landed (run `b8b3e14f-b202-4015-b7d9-1ef15efb820d`),
  showed the app-bar test above and a launcher search test fixed in launcher-enter-feedback.
- Skipped: the workspace and desktop doctests (no plan adds or changes a documentation example),
  `cargo check -p farhelm-desktop` and the desktop smoke test (the desktop app's own crate is unchanged), the native Mac
  app (not runnable here) and the installer (unchanged).

The report's checks ran on a main from before round 109's landings, so they no longer describe what landed; the combined
runs above supersede them. Nothing else in the executor's report became untrue at landing.
