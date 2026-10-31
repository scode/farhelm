## What this was about

The Templates list's bolt looked filled whether marked or unmarked, making the quick-launch template hard to identify.
Save also looked available after saving, leaving completion unclear. The maintainer chose an outline unmarked bolt, a
filled amber marked bolt, and Save greyed out whenever the editor has no unsaved changes.

The two draft PRs implement those choices. The bolt keeps its size and position when its shortcut label appears. Save
refuses clean activations, becomes available after an edit, and retains keyboard focus after saving. Both TODO entries
are removed.

## Things you should know

The leave prompt's Save, the existing busy lock, and other uses of the bolt character are unchanged. Normal browser
screenshots were inspected; no video or special capture harness was added.

The source-only sleep annotation checker did not finish: three attempts exceeded its 60-second source-check budget.
These are incomplete checks, not passes. No delay calls or process-environment changes were added. An earlier lint
attempt exited 143 with no diagnostic; the final targeted lint passed.

## Open questions and possible follow-ups

An older template with no stored "switches launcher to" choice is shown as an unsaved draft when selected in Templates:
the editor infers Agent or Command, so Save is available and leaving can ask to save even without a hand edit. The
editor opened automatically after "Save as template" instead starts from a clean draft. These existing entry-route rules
remain; this work does not normalize them. The sleep checker needs a completed run when its source scan can finish
within its budget.

## The PRs

- [#2844: distinguish the quick-launch template bolt](https://github.com/scode/farhelm/pull/2844/changes), head
  `596ec9d4d9efb4b0e96778e270c57720715b5981`.
- [#2845: show when a template needs saving](https://github.com/scode/farhelm/pull/2845/changes), head
  `6e7457f4d7f29a86876642197a0013fae82e24a7`, based on #2844.

## Checks run, reused and skipped

- Passed `cargo build -p farhelm`, both release web builds with
  `dx build --package farhelm-ui --platform web --release`, `cargo clippy -p farhelm-ui --lib -- -D warnings`,
  `cargo fmt --all -- --check`, configured Markdown formatting, and `python3 releasing/check-changelog.py format`.
- Recorder `3a309a01-b066-45ed-8c73-4b29a375883f`: all 17 CSS-token tests passed, including both palettes. Reused for
  the final stack because PR2 changes no stylesheet.
- Recorder `91300ce8-4a56-4df4-beec-eece769568ea`: 8 browser cases passed, four per engine, covering the bolt drawing
  and existing mark/rename/delete behavior. It covers #2844 and remains applicable because PR2 changes neither the bolt
  nor its action.
- Recorder `799110b3-6859-42f4-8d22-d020d89e6e44`: 28 browser cases passed, 14 per engine, covering Save state and
  focus, busy keyboard isolation and scrolling, create/edit/rename/undo, legacy names, unsaved departures, phone
  controls, custom permissions, and number-menu movement. It covers #2845's product behavior before the final
  screenshot-only test refinement.
- Recorder `82fb2c1d-52e5-4271-9923-293b6f5385c6`: the refined Save test passed in both engines, explicitly checking its
  settled neutral fill before the normal screenshot. No product code changed after the 28-case run. All browser runs
  used pinned tmux, three worker slots, and zero retries; no failures or skipped cases occurred. Chromium and WebKit
  screenshots were inspected.
- Incomplete: `python -B scripts/check-test-sleeps.py`, as described above. Skipped the full browser, backend, and
  native-desktop suites because the selected shared-UI checks cover the changed drawing and editor guard. No published
  documentation shots were refreshed.

## Review gate outcome

Each PR passed its required fresh gpt-6.1-sol/high adversarial review. The first review caught vertical movement when
the shortcut label appeared; the icon was anchored and the test now checks its position within the row. The reviewer
verified the correction and the test cleanup fix. PR2 had no findings, and its settled-color test refinement also passed
review. Both commit messages passed independent cold reads. No findings were rejected.

### Landing

Landed as delivered, with no fixes at landing. In the Templates list, the ⚡ quick-launch mark is now clearly lit only
on the marked template (an outline elsewhere), and the template editor greys out Save when the draft has nothing unsaved
(it already showed "Unsaved changes" when it does), so it is visible whether a save is needed and whether one went
through.

- #2844: the list's ⚡ states and their hover text.
- #2845: the editor's save state, tests, the specification text and changelog entries.

#### What landed since the stack was based, and whether it interacted

What landed on main since (the navigation rail rename, #2849, a docs refresh and a README screenshot refresh, #2851 and
#2852, and TODO entries) did not interact; the rebase onto it was clean. host-rail-list-visible landed right before this
plan in the same round, with its own fix at landing; the two share nothing but bookkeeping. command-shell-meaning, sent
back this round, will make a template's wrapper words mean what they would in a shell; a review checked that the
editor's save state compares those words correctly either way.

#### Things you should know

- Older templates saved before the "switches launcher to" choice existed open as "Unsaved changes" with Save lit (the
  executor's report lists this), and so, the landing review found, do templates with an old trust setting.
- A new, untouched draft now starts with Save greyed, so pressing it does nothing rather than refusing the empty name.
- A greyed Save keeps its ordinary hover text and does not say why it is greyed; the Save button for a checkout's
  settings behaves the same.
- The next docs-screenshot refresh will show a greyed Save in the template editor picture.

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

The report's checks were the executor's selected runs, which left out most of the templates and quick-launch specs; this
landing ran both in full. The report lists the test-delay check as unfinished; the landing ran it and it passed. Nothing
in the report became untrue at landing.
