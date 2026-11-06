# One Launch press for a fresh checkout on a suspended host

## What this was about

Starting a fresh GitHub checkout on a suspended Tensorlake host used to resume the host, then require another Launch
press to review its new directory. The maintainer chose automatic allocation after the host wakes, with no directory
review. The session launcher now resumes and launches from one press, including quick-launch and number-key templates.

## Things you should know

Before Launch, the suspended-host checkout shows no directory path. It says the directory will be picked when the host
is ready, or named after the session when a name is supplied. The launch waits for a fresh connection and a current
checkout preview; a preview from before suspension cannot authorize it. A template also keeps its accepted launch when
the host's checkout settings finish loading after the shortcut was pressed.

Unnamed checkouts, template base names and untouched Clone names keep their existing allocation and numbering rules. A
user-entered or fixed name whose directory is taken stays refused. If another launch takes the checked directory, the
launcher reports the conflict and requires another click; it never automatically retries a refused create.

The incidental documentation changes are on the Start a session and Tensorlake sandboxes pages. No preview server was
started for this unattended run. The matching TODO entry was removed and a changed changelog fragment was added.

## Open questions and possible follow-ups

No known outstanding behavior questions or required follow-ups.

## PRs

- [#3073](https://github.com/scode/farhelm/pull/3073/changes) — one-press resumed checkout launches; draft, based on
  main, head d11009dc8719790698d426de7de2dadd4a76b503.

## Checks run, reused and skipped

- Passed the final recorded browser selection below: 32 cases, 16 Chromium and 16 WebKit, zero failures or skips, three
  worker stacks and no retries, with pinned tmux. Run f59530ec-fbe8-4ca5-8e0f-b8038e3196b0 covered the same source tree
  committed in #3073. The new lifecycle cases control resume, preview and create replies to verify dispatch; the
  connected-checkout cases perform real clones and directory collisions in owned local fixtures.

  ```sh
  npx playwright test tensorlake-lifecycle.spec.ts github-checkouts.spec.ts -g 'suspended fresh checkout uses one press|quick launch (waits for a managed preview|numbers a template base|shows preparation)|launching in an existing folder|successful resume stays honest|failed resume refuses|delayed create after resume'
  ```

- The earlier combined browser run 9dc64e88-1d08-45b8-b414-ebf8a11dae72 passed 30 cases and failed the two WebKit
  template cases. Loading checkout settings after the shortcut cancelled its accepted launch. An isolated run on a fresh
  configuration passed, then the tests were made to force that ordering: reproduction
  ac88412b-d68c-4d5c-a223-a132569da1c2 failed both cases before the correction, and aa089577-a95e-4506-b6b9-2cef2a34b56c
  passed both afterward. Failed evidence is retained; this was fixed in this run.
- Passed `cargo nextest run -p farhelm-ui --lib -E 'test(list::create_form::) | test(github_checkout::)'`: 117 selected
  tests, run d4239298-4072-4edf-8b9a-4a1ee4486cc9, on the working tree based on 8622676b with source fingerprint
  636213a37d34bc2ce0d47edcc3c3397e24eefe0f4676842a4240db93a03dfd0a. Reused for the unchanged launcher, naming and
  preview logic: the only later Rust change was the resumed-stamp configuration comparison and its assertions. That
  final correction passed `cargo nextest run -p farhelm-ui --lib -E 'test(/(guarded|resumed)_submit_stamp/)'`: two
  tests, run 310b7eaf-2f56-4a70-a3e4-28aa41e9b417. Both used the recorder, four slots, no retries and pinned tmux;
  selected-out tests are excluded from these pass counts, and no selected test reported a runtime substrate skip.
- Passed `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the
  application build and the Dioxus 0.7.10 release web build.
- Passed Rustfmt, dprint on the changed Markdown, the test-delay checker (413 delays, zero missing explanations), and
  `python3 releasing/check-changelog.py format`.
- Skipped the full test battery, hosted CI, native desktop runtime and real Tensorlake calls: this changes launcher
  logic, covered by focused unit/browser cases and desktop compilation. WebKit checks the engine family, not a native
  macOS window. The docs edits change prose only, so no website build was needed.

## Review gate outcome

The single required fresh-context gpt-6.1-sol/high reviewer found one issue: Enter on a recent GitHub setup could submit
before the launcher rerendered. That was corrected and covered by a browser case. The same reviewer accepted that
correction and the later settings-discovery correction with no remaining findings. A separate own-model scope assessment
confirmed that the latter fits allocation after resume without weakening current-preview validation. Commit and PR
wording passed the cold-reader process. Main's subsequent change only blocked another plan; it changes no product code
or shared behavior, so no rebase or additional runtime validation was needed.

### Landing

Landed with two wording fixes at landing. Starting a fresh GitHub checkout on a suspended Tensorlake host now resumes
the host and starts the session from one Launch press, including from quick launch and number keys. Before the press the
launcher shows no directory, only that it will be picked when the host is ready; a preview from before the suspension
still cannot authorize the create.

- #3073: the launcher change, its unit and browser cases, the specifications, the Start a session and Tensorlake
  sandboxes guide pages, and the TODO entry it addresses.

#### What landed since the stack was based, and whether it interacted

Nothing but plan bookkeeping landed on main since the plan's base, and nothing else landed in this round. A
fresh-context review read it against the changes that landed shortly before its base and touch the same areas: the
Tensorlake lifecycle tests' fake hosts and their saved evidence at known flaky checks (#3062, #3069), the folder
dropdown and shared-checkout tests (#3063, #3059), the Add host hover text (#3067) and the Tensorlake launcher work this
plan builds on (#3048). None of them conflicts with it: the new browser cases use those fake hosts as they now behave,
and ordinary launches that answer a permission question behave as before.

#### Fixes made at landing

- The changelog: Tensorlake hosts have not been released yet, so this follow-up belongs in their existing entry rather
  than in an entry of its own (`AGENTS.md`, "Changelog fragments"). The plan asked for a separate "changed" entry, but
  it also names that rule as binding, and the existing entry still said a fresh checkout asks you to review the preview
  and launch again. That sentence now describes the one-press launch, and the separate entry is gone.
- The Start a session page said the launcher always shows the clone's directory before you launch, with quick launch and
  number keys as the only exceptions, which contradicted the same page's new text about suspended hosts. It now says a
  suspended Tensorlake host has no directory to show yet and that one press resumes, picks it and launches.

#### Things you should know

- If the suspended host has no recorded installation identity, pressing Launch on a fresh checkout does nothing and says
  nothing; with quick launch the shortcut then fails silently. A connected host in the same state says "The host
  installation is not verified". A Tensorlake host should always have one, so this is unlikely; showing that message
  here too is a possible follow-up.
- The composer hides the directory only while the host reads Suspended. While it resumes and reconnects, the old
  connection's directory can show briefly before the fresh one replaces it. That is display only: the old preview still
  cannot authorize the create.
- **Replace with** into a fresh checkout on a suspended host now deletes the source session in that same press, with no
  directory shown first. That follows from your choice of no review step, but no test covers this combination; a browser
  case for it is a possible follow-up.

#### Checks

This round landed only this plan, rebased onto main `fdacdba6`. Before the two wording fixes:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 413 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older text), the docs website build (`bun run build` in `website/`), the workspace debug build and the
  UI's release web build: all passed.
- The UI's JavaScript tests (`node --test` in `crates/farhelm-ui/js-tests`), run `b2b39a75-8951-43fb-8513-07b6c3fd0efa`:
  all 527 passed. The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `6b57fd62-6e3c-4e57-acd2-a46a7a4ce2d5`:
  all 897 passed; its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `76907fe7-844a-4ee3-b56c-885984f08fc3`: all 1086 passed.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each started its own three copies of Farhelm to test against and ran its share of the suite's spec files
  through the test recorder, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack; the sandboxes run Node 24.16 rather than the development machine's 24.20), runs
  `fd92f800-4245-4984-a1e4-fae5664defc3`, `9d02a38a-45b3-44f0-b5a4-fca19cff251e`,
  `269f1dd3-f429-4b1d-9171-523f76ef94d9`, `b9c666cd-8863-4040-a7da-23a190239eff`,
  `d968a957-365b-4dd9-be06-f7c83a9f5ffc`, `2cc4c491-1c65-4e1d-ba77-51e339e3de8a`,
  `62b04a5b-a772-4e81-91d9-83eb2cb858da`, `9b2ccf68-84da-4067-81dd-155e1a8acfb3`,
  `a63b1e34-599c-4a38-9062-f58fe384ef09`, `12fb9421-d6d3-43fc-9030-5154d715e58d`: 3095 passed, 18 skipped by the specs'
  own design, 1 failed. Then the same suite with every stack's terminals in terminal hosts (this run covers only the
  specs listed in `e2e/host-terminal-specs.ts`, so it holds fewer tests), runs `01891ff5-400a-4746-98bd-336520219baf`,
  `7c5bc684-b354-424c-8b1b-3698a25b3789`, `0496936d-f186-43c6-ada6-b83b1b131aca`,
  `5c8ca078-e56e-4418-a734-0c9ba4d27fcf`, `f336e05e-a17a-442a-aa9a-1b243610e112`,
  `f96f607f-c4ee-4129-9e02-ae54b1193651`, `4e4ca202-0f2f-47ce-85ec-8480d5170294`,
  `40e6f124-6214-4bd7-865c-64740aecd10e`, `5b1334bd-9819-4c60-9228-a8c0eb7a8157`,
  `3733b866-6b06-4301-9c0f-695eadd6aba1`: 2111 passed, 75 skipped by the specs' own design, 4 failed. Of the five
  failures:
  - "Incarnation churn keeps a followed run while a target change detaches it" (WebKit, run `269f1dd3`): an update's
    progress label showed no step name within five seconds, the latent flake recorded in `FLAKES.md` on 2026-11-05 for
    three sibling provisioning tests. It passed three re-runs on the same build, run
    `2bbe5a10-4b0f-4673-a93b-c14151d5598b`. The landing monitor adds this test to that flake's record, in a separate
    change made right after this landing.
  - "Composer saved defaults clear explicit choices without posting" (WebKit, in the host-terminal suite, run
    `01891ff5`): the model picker's option never appeared. It passed three re-runs in each suite, runs
    `4b5e33af-ddc3-4cba-b618-258763b5795a` (host terminals) and `4081e7e3-64ff-450c-8914-763184d5b803`. The test picks a
    model in the launcher's ordinary form, which this plan's suspended-host path does not reach, so the failure is most
    likely unrelated; with one failure in seven runs it is not recorded as a flake.
  - "Trash header and global confirmation remain reachable at width 360" (Chromium, in the host-terminal suite, run
    `4e4ca202`), a recorded flake with the same signature; not re-run.
  - "Post-clone commands with carriage returns refuse edits but can be cleared" and "a pending checkout folder save
    survives closing settings" (WebKit, run `40e6f124`, host terminals): each timed out in its cleanup, waiting for the
    browser to report a save's request finished. They passed three host-terminal re-runs (run
    `cb57caa4-7c10-4a8b-9a8f-157331af8daa`) and failed once each in three ordinary re-runs (run
    `b9bc59c5-c98f-4e0d-92fc-6735b2c9fc50`), in the same place. #3061 found that WebKit can leave that event pending for
    an intercepted save and changed two sibling tests to stop waiting on it; these two still wait on it. This plan does
    not touch Settings. The landing monitor records them as latent flakes in a separate change made right after this
    landing.
- After the two wording fixes: `dprint check`, `python3 releasing/check-changelog.py format` and the docs website build
  again: all passed. The fixes change only a changelog entry and a guide page, so nothing else was run again.
- Reused: none. Not run: the other crates' Rust tests (only the UI crate changed), the doctests (no doc examples
  changed), the grid component's browser suite (no grid code changed), the desktop smoke tests and the installer tests
  (no desktop shell, packaging or installer code changed), the docs website preview (the build above passed; no page was
  viewed), real Tensorlake calls from the launcher (the browser cases control the resume, preview and create replies)
  and the native Mac app (not runnable on Linux).

Nothing in the executor's report became untrue at landing, except that it names a separate changelog entry, which the
landing folded into the Tensorlake hosts entry, and calls the PR a draft; what landed is its commit with the two wording
fixes, on current main.
