## What this was about

Hovering over a host widened the rail over the sessions it was meant to filter, while the controls at its foot stayed
narrow. The agreed replacement keeps full names in tooltips and adds a button that deliberately expands the whole rail
beside the session list.

## Things you should know

Each device starts collapsed and remembers its own choice. The button below Help expands the rail and its foot from 56px
to 220px. Expanded names are clickable parts of their host rows; choosing a host keeps the rail open. Tooltips and
accessible names include a distinct remote destination and retain the existing state explanations. Hover and keyboard
focus no longer widen anything.

Expansion adds up to 164px to the sidebar, taking room from the terminal while preserving the session list's width when
space permits. The approved cap limits that growth in smaller windows; the list supplies the remainder. The stored
resize width keeps its old meaning. At the cap, dragging can change that stored width before the visible edge moves, as
the plan accepted. Very narrow windows retain horizontal scrolling. Status choices wrap when the expanded list becomes
too narrow, including at the minimum stored sidebar width.

The specification and changelog describe the change, and the TODO entry is removed. README, video and docs screenshots
remain for their next requested refresh; the fenced media was not changed.

## Open questions and possible follow-ups

No maintainer decision is needed to land this work. The next normal media refresh can pick up the fourth footer button.

## The PRs

- [#2841 — replace host rail hover with an expand button](https://github.com/scode/farhelm/pull/2841/changes), draft.

## Checks run, reused and skipped

Browser commands ran from `e2e` through this recorder wrapper, with pinned tmux on PATH:

```sh
python3 ../scripts/record-test-run.py --runner playwright --kind development --tmux required --timeout <seconds> --selection '<described selection>' --concurrency '3 browser workers, one stack each; retries 0' -- npx playwright test <arguments below>
```

Each selection used both configured engines, Chromium and WebKit. The portable child arguments and command deadlines
were:

- Initial rail/filter selection (900 seconds): `'host-short-names\.spec\.ts' 'filters\.spec\.ts'`.
- Broader rail/footer regression selection (1800 seconds):
  `'sort\.spec\.ts' 'hosts-panel\.spec\.ts' 'sidebar\.spec\.ts' 'host-updates\.spec\.ts' 'host-rail-warning\.spec\.ts' 'settings\.spec\.ts' 'tooltip-coverage\.spec\.ts'`.
  The settings suffix also selected `ntfy-settings.spec.ts`.
- Exact narrow-window case, both the failure and passing rerun (300 seconds):
  `'filters\.spec\.ts$' --grep 'the rail keeps keyboard focus and fits a narrow window$'`.
- Final continuation (1200 seconds):
  `'sidebar\.spec\.ts$' 'host-short-names\.spec\.ts$' 'filters\.spec\.ts$' 'status-filter\.spec\.ts$' --grep '<generated expression selecting the 40 unfinished sidebar titles or any case in the other three specs>'`.
  The generated expression is redacted; its exact selected cases are retained with recorder
  `137fce39-32a5-4135-8828-1213d3b153dc`.

- Final browser continuation, recorder `137fce39-32a5-4135-8828-1213d3b153dc`: all 130 passed, 65 per engine, zero
  failed, skipped, flaky, interrupted or unstarted cases. Complete source, reports, output EOF and normal cleanup were
  confirmed. It ran the complete host-name, host-filter and status-filter specs plus the 40 sidebar titles left
  unfinished by the earlier command deadline, on both Chromium and WebKit, three workers, zero retries, including
  intentional repeats of the already completed Chromium sidebar cases. The final roomy expanded and collapsed captures
  were also inspected.
- The preceding rail/filter selection, `40d4c28f-394d-4961-b86a-8ee994d70329`, passed 38 cases. Its captures exposed
  overlapping narrow status labels that its original assertions missed. Stronger bounds checks then caught selected
  Running extending outside its well in WebKit at the minimum width: `58557e12-8d0c-4ab9-9a42-5c17aab600f6`, one pass
  and one failure. Expanded-only wrapping and reduced label padding address those observations. The final exact case,
  `37402946-51d9-4cd6-9a21-289e67a5527d`, passed on both engines, including selected-font checks at 800, 750, 718 and
  360px windows and the 296px stored minimum. All six final constrained captures were inspected.
- The broader rail/footer regression selection, `7bb43f5a-2500-49ee-9622-199effaacf7c`, reached its 30-minute command
  deadline with 368 passes, no failed assertions, one interrupted WebKit case and 39 unstarted cases. It was not a
  passing selection. It covered sorting, host management, sidebar, host updates and warnings, settings and notification
  settings, and hover help; the continuation above selects precisely its unfinished titles as well as the final rail
  checks. The timeout record and interrupted trace remain retained.
- Two attempts, `37609094-43f3-4176-9af3-d8fd4189c263` and `ee6e947b-14fb-4dc6-8860-51429f1a0860`, refused before
  browser startup because the Playwright version probe did not finish. Direct installed-version probes succeeded and
  unchanged retries ran. These contain no test results. Their records and the same-session WebKit failure remain
  retained; no latent flake is claimed.
- From `crates/farhelm-ui/js-tests`,
  `python3 ../../../scripts/record-test-run.py --kind development --selection 'UI asset JavaScript with final expanded label padding' --concurrency 'node default discovery' --tmux none --timeout 60 -- node --test`,
  recorder `dd35a0c8-5457-4dd2-9ac9-2e7a346f8303`: 383 passed, zero skipped. This includes the expansion math, device
  storage, mount/remount ownership and stylesheet checks. Earlier iterations passed 381 cases, then 383 after upstream
  added two shortcut tests. The final UI release web build passed with the pinned Dioxus 0.7.10 builder:
  `dx build --package farhelm-ui --platform web --release` from `crates/farhelm-ui`, with the pinned builder selected on
  PATH. Two initial invocations selected an unrelated executable and stopped before building; the correct builder was
  used thereafter.
- Focused UI Rust tests, `cargo nextest run -p farhelm-ui --lib -E 'test(/rail_|footer_menu/)'`, recorder
  `0145f3fd-50ac-4371-b7c6-4dee5bde4944`: three rail identity/presentation/update tests passed, 823 outside the
  selection. No footer-named unit matched; footer geometry is browser evidence. Pinned nextest/tmux, four slots, zero
  retries; retained output contains no runtime SKIPPED notices.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check SPEC.md SPEC_impl.md releasing/changelog.d/host-rail-toggle.md`,
  `python3 releasing/check-changelog.py format` and `<isolated-python> -B scripts/check-test-sleeps.py` passed. The
  isolated interpreter's private location is redacted; the delay checker inspected 327 delays with zero missing
  explanations. `python3 scripts/check-agents-md.py` reported zero errors and three pre-existing citation warnings.
- Final JS and focused browser evidence covers the uncommitted UI on base `cac2d3bb`, fingerprint
  `5c19210957b0b8dbf65231e50367853b80013c6fc5385c320b7ddae2d70db45c`. The same rail changes were committed as `3501c33e`
  after rebasing onto `35d8321f`. The intervening change adds cleanup diagnostics with unchanged operations, locks and
  replies. The earlier Rust tests and UI lint cover the feature's unchanged Rust on base `fcad8cbb`; the Rust run
  fingerprint is `b12c3aff260a9183338cc9e721cfe48326f759c0623a5fe78f8aee9b2f448ca4`. The browser regressions before the
  correction cover `fcad8cbb` with fingerprint `cd486848e761fecbdbaaed314a13bf9f5124deef18e77e68b6e7980bfc3b6fe0` and
  are reused only where behavior stayed the same. The rebased UI was rebuilt and keyboard activation of the rail toggle
  is in the final selection. The server build (`cargo build -p farhelm`) and fixture build were reused because the rail
  APIs and fixture contracts are unchanged; upstream template-name validation and cleanup logging were inspected
  separately, not claimed as newly tested backend work.
- Whole-workspace, installer, provisioning, desktop smoke and native macOS runs were skipped: this change concerns
  shared rail markup, CSS and page-owned storage, covered by the targeted checks and both browser engines. No website
  page or generated media changed, so no website or media build was needed. No CI workflow was dispatched.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design, idioms and the full
test-authoring checklist. Its findings caught footer icons and menu anchors shifting during expansion, and text-overflow
measurements that could miss a failure while the intended font was still loading. Both were corrected and verified; the
final padding correction was also inspected. No source findings remain. A fresh own-model scope reassessment kept the
narrow fix to the existing wrapping facility rather than a new layout mechanism. Commit and PR wording passed a separate
cold read, and the documentation pass is complete.

### Landing

Landed with one fix at landing (below). The sidebar's left column of host icons no longer widens over the session list
when the pointer rests on it or keyboard focus enters it. Full host names and destinations now show in hover text, and a
new button below Help expands the whole column beside the list instead of over it; each device remembers its choice.

- #2841: the expand button and its remembered state, removal of the hover and focus widening, tests, the specification
  text, a changelog entry, and removal of the TODO entry that asked to revisit the widening.

#### What landed since the stack was based, and whether it interacted

The column was renamed the "navigation rail" on main (#2849) after this plan was based, with a changelog entry that
announces the new name in the same release; that is the fix below. During this round a docs refresh and a README
screenshot refresh (#2851, #2852) also landed; the rebase onto them was clean, and nothing else interacted. This round
also checked tab-output-hangup and command-shell-meaning, which went back to their executors, and
template-bolt-and-save, which lands right after this plan.

#### Fixes made at landing

- The new button said "Expand host rail" / "Collapse host rail", the old name. Its labels and hover text, the SPEC.md
  sentence quoting them, the tests pinning them and the changelog entry now say "navigation rail". Internal names and
  SPEC.md's other mentions of the host rail stay, as #2849 left them.

#### Things you should know

- In a window too narrow for the sidebar to grow (about 620px or less), the expanded rail leaves a 76px session column;
  and at the window's limit, dragging the sidebar edge changes the stored width before the visible edge moves. The plan
  accepted both.
- A tooltip already open on the button keeps its old text until it is shown again.
- One leftover style rule no longer does anything.
- The docs refresh that landed during this round (#2851) says on Manage hosts to rest the pointer on the rail to see
  full names; that still holds (they are now hover text), but no docs page mentions the new expand button yet.

#### Checks

The round first checked four plans stacked on main `8754d34f`: this round's two landing plans, and tab-output-hangup and
command-shell-meaning, which went back to their executors. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint, the binaries' lint,
  the desktop-build checks, the check that every test delay states its reason (`python -B scripts/check-test-sleeps.py`,
  327 delays, none without a reason), `dprint check`, `python3 releasing/check-changelog.py format`, the docs website
  build, the workspace debug build, the UI's release web build and `scripts/check-desktop-assets.sh`: all passed.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `cf586b3d-ea51-4690-886b-2aa9548bf91e`: all 833 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `6d1750da-8ae8-4a2e-8ce3-39a71fe9fa83`: all 1016 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `07ad99dd-9259-406b-a620-4c2108aa3aea`: all 383 passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, run
  `7ec0376d-1c10-4848-877a-ec3ea8d4c44c`: 4615 passed, 41 left out by the test runner, 2 failed, both
  command-shell-meaning's own tests (it went back for them).
- `npx playwright test` in `e2e/`, the whole browser suite on Chromium and WebKit, three workers, no retries, run
  `afafdf3a-2c2c-4d7f-8ef0-28e9e3706ad0`: 2839 passed, 14 skipped by design, 11 failed, while the machine was heavily
  loaded (2.6 hours). Every spec with a failure was run again below.

With host-rail-list-visible and template-bolt-and-save alone on main `8754d34f`, after the navigation-rail fix:

- `dprint check`, `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`,
  `cargo check -p farhelm-ui --features desktop`, the web-target UI lint, the workspace debug build and the UI's release
  web build passed. The UI's Rust tests, run `ac263c0a-bac9-4c61-97bb-cd3a00842f8f`: all 833 passed. The UI's JavaScript
  tests, run `4f7454e0-248c-4f7d-a39d-bd7bbba82ce5`: all 383 passed.
- At the maintainer's request, the browser specs ran on ten Tensorlake sandboxes (cloud virtual machines, not the usual
  development machine) using the binaries and web bundle built locally from this pair. Each sandbox booted the usual
  three test stacks and ran its share of the projects through the recorder
  (`python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, the recorder's generic mode, because its Playwright
  mode cannot select one engine). The selection was 21 specs on both engines: the plans' own (host short names, sidebar,
  sidebar resize, templates, template list shortcut, quick launch, launcher template shortcuts, the hover-text sweep,
  filters, sort, Hosts panel, settings) and every spec that failed above (terminal link clicks, feedback, GitHub
  checkout composer, GitHub checkouts, header, launcher Enter, modal layout, Replace, provisioning). Across the ten runs
  (`4ebee4f8-3271-4bad-9c44-2ec88c155acc`, `a88d1d9a-7e6c-4b78-a00f-197dc8051020`,
  `845df979-0649-477f-be13-a122fe0cca2d`, `29a48694-8549-4f87-88d3-c0fafc480b84`,
  `0f1c4576-5530-4fd7-bec6-c9c74e3d0d03`, `d6db4aaa-c0b1-47f1-9573-30a6a956755c`,
  `e948da86-d374-4f9a-bbf5-9b1b1e81f8fe`, `8845e6fb-966f-4eba-8ea9-9ab1e814ace3`,
  `02911f73-2e0b-4a38-816c-4986d9f2670f`, `058fb81e-f3f8-4e1d-9b94-a6e605ccea7a`; only the two failed runs' records were
  kept, the rest went with their sandboxes), 1174 passed and 2 failed, both on Chromium. "a wrong-host UPDATE acceptance
  warns on its source row and claims nothing" (provisioning) has failed in earlier rounds too, on main without these
  plans among them, and an open draft PR records it as a flake. "typed page refusals are atomic and empty Enter leaves
  later bindings paired" (quick launch) is a single observation. Re-run three times each on both engines with `-g` and
  `--repeat-each=3`, all 12 passed (runs `c7c7d0da-96a8-4fbd-b6f0-23bb6dd6d65d`, `e47e4345-6e33-42ef-9d8c-3670f9345851`,
  `b620e1ab-4a59-429a-b91c-55774a0cb055`, `59d04d10-b37b-4d88-a969-d89205166bbf`). The sandboxes ran Node 24.16, against
  24.20 locally.
- Reused from the four-plan run: the whole Rust workspace's passes, because the pair changes only the UI crate, whose
  tests were run again above, and both of that run's failures were command-shell-meaning's own tests; the desktop-build
  tests, since the pair changes no desktop-specific code and its desktop build was compiled above; and the browser suite
  outside the specs above, since no other spec checks the renamed button's text. Not run: the desktop smoke tests and
  the native Mac app (not runnable on Linux).

The report's checks were the executor's selected runs; this landing ran the full battery with all four of the round's
plans stacked, and the affected specs again on this plan and template-bolt-and-save alone. Nothing in the executor's
report became untrue at landing beyond the button's name.
