## What this was about

Hovering a session showed a summary card over neighbouring sessions, making the list harder to browse. The card now
appears six pixels to the right of the sidebar, level with the row, and moves up when needed to fit the window. This
applies to hover and keyboard focus in both list densities, on web and desktop.

## Things you should know

A window without enough room beside the list keeps the previous above-or-below placement. A showing card notices a
window resize even when its row does not move. Ordinary tooltips, the host-update progress popup, card contents, timing,
dismissal and accessibility keep their existing behavior.

The specifications and changelog describe the placement, and the source TODO entry is removed. No published screenshot
or release capture changed. Terminal host recovery landed during execution; the change was rebased onto it without
conflicts and with an empty comparison against the reviewed patch. Subsequent checkpoint-stop changes affect separate
supervisor functions and specification paragraphs, so they did not require another rebase or tooltip test run.

## Open questions and possible follow-ups

None identified. Native macOS appearance was not checked; WebKit browser checks cover the shared placement script.

## The PRs

- [#2972: move session summaries beside the list](https://github.com/scode/farhelm/pull/2972/changes) — draft,
  `plan/session-summary-beside-list/01-card-beside-list`, head `dce09156e385628a3d6d9768deedd9435625ecf3`.

## Checks run, reused and skipped

Run for this change:

- From the UI JavaScript test directory, recorder → `node --test`: 512 passed, none skipped, run
  `a08b938a-87e1-4a6f-a29f-bc7af031b986`. This includes the four new pure placement cases.
- From `e2e`, controlled recorder → `npx playwright test 'tooltip\.spec\.ts|popover-identity-movement\.spec\.ts'`: 50
  passed, 25 each Chromium and WebKit, none skipped, run `a8ecf416-ef74-4c31-acb4-3e80bdab6924`. Checks cover both list
  densities, the resting-pointer resize fallback, ordinary tooltip placement and existing popover movement.
- With `FARHELM_E2E_SUITE_TERMINALS=host` intentionally retained through the recorder, the same browser selection: 50
  passed, 25 each Chromium and WebKit, none skipped, run `069c9fe0-1dad-4a04-b04c-fbab481596cc`.
- `cargo clippy -p farhelm-ui --all-targets -- -D warnings`, `cargo check -p farhelm-ui --features desktop`,
  `cargo fmt --all -- --check`, and the release web UI build passed.
- `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/session-card-beside-list.md`,
  `python3 releasing/check-changelog.py format` and whitespace checks passed. The isolated interpreter's
  `python -B scripts/check-test-sleeps.py` inspected 397 delays with no missing rationale.

Both browser records have complete source identity and output, validated runner policy, zero retries and the matching
pinned tmux 3.7c. The final JavaScript run, lint, desktop compilation, web build and formatting evidence from the
reviewed working revision `02853b81` remain applicable after the rebase: the patch comparison was empty and the UI
source and assets are identical. Markdown and whitespace checks also passed on the rebased tree. The existing CLI and
fixture binaries were reused: the intervening backend change concerns recovery from terminal host failures, which these
healthy-stack tooltip scenarios do not exercise. This is placement evidence, not recovery coverage.

Skipped the wider Rust, browser, installer and release suites because the change adds one row attribute and shared
tooltip placement, covered by compilation, pure-function and page tests. Skipped the staged-fleet visual capture because
the actual browser geometry assertions cover the placement and no documentation screenshot needs refreshing. CI was not
dispatched; this repository runs it only on demand.

## The review gate's outcome

The required single fresh-context gpt-6.1-sol reviewer at high effort found no issues in correctness, design, idiomatic
code or the acceptance criteria. Its prompt included the full goal and test-authoring rules. The separate documentation
pass covered every touched file. A fresh wording reader understood the exact commit and PR text with no contradictions
or convention issues. No scope departure or corrective review round was needed.

### Landing

Landed with no fixes of its own, last of the three plans landed together in this round. Hovering or focusing a session
now shows its summary card to the right of the sidebar, level with the row, instead of over the neighbouring sessions.

- #2972: the card's placement, its tests, the specification and the removed TODO entry.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained faster-snapshot-encoder, colored-harness-marks, discord-link-in-app,
grid-drop-and-drag-notice, and PRs that only added TODO entries and plans. In this round, launch-progress also edits the
session row; a fresh-context review found the two in separate places (its launch-progress notes say how), and
colored-harness-marks' colored marks in the same row do not affect where the card goes. The rebase conflicted only in
implementation-notes sentences changed apart, resolved keeping every side's changes; a check confirmed the commit
changes exactly the sentences it changed when tested.

#### Checks

This round landed vs16-emoji-width, launch-progress and session-summary-beside-list, stacked in that order on main
`5c5af756`. On that stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the web-target UI lint,
  `cargo clippy -p farhelm --bins -- -D warnings`, `cargo check -p farhelm-ui --features desktop`, the check that every
  test delay states its reason (`python -B scripts/check-test-sleeps.py`, 403 delays, none without a reason),
  `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three
  warnings about older reports), the docs website build (`bun run build` in `website/`), the workspace debug build and
  the UI's release web build: all passed. These ran before launch-progress's landing fix, a stylesheet value that
  computes to the same style; the builds were redone with it for the JavaScript run `c5047ec6`, the trash dialog runs
  `d30273b9` and `03eda759`, and launch-progress's own specs below.
- The UI's Rust tests, `cargo nextest run -p farhelm-ui`, run `b14ee80d-08f8-4500-8471-e106fd12b13d`: all 842 passed;
  its desktop-build tests, `cargo nextest run -p farhelm-ui --features desktop`, run
  `b4981abf-b680-4336-8ed1-c1b993d4f947`: all 1031 passed. The UI's JavaScript tests (`node --test` in
  `crates/farhelm-ui/js-tests`), run `83ad6a8c-ae24-43a4-a2a5-be982d905cb0`: 515 passed, 1 failed (the stylesheet test
  launch-progress's landing fix addresses); with the fix, run `c5047ec6-c334-493a-b9d8-4ad22ecd5b84`: all 516 passed.
- The vendored crates (vs16-emoji-width patches them): `cargo test` of vendored vte with `--features ansi`, run
  `c1feddd1-8351-46cd-a4dd-fe2a3c2c34d6` (62 passed), of vendored alacritty_terminal, run
  `ac66eb6f-33df-414f-8023-8e6f82d808ac` (165 passed), and of the paired tests, run
  `11056c7f-5f7e-41fb-8052-54cb6fdb7f15` (3 passed). The terminal emulator's ignored round-trip and hostile-snapshot
  tests in release (`cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E '...'`, the
  full-size round trips, the 300,000 short scripts and the 230,000 hostile blobs), which their documentation asks for
  when the decoder or the vendored patches change, run `c957bc92-29df-4bec-9178-06713631da5c`: all 10 passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux first on `PATH` (so the emulator's parity tests against tmux ran), no retries, on a machine with rootless Podman,
  run `68783337-311c-45d3-b891-a1dfc3eb9f29`: 5428 passed, 70 left out by the test runner (tests marked ignored), 3
  failed: two restart-gap session-listing tests and an OMP report test, which have failed the same way on main since
  round 169 found them (stop-checkpoint-burst's landing notes name them), in code none of this round's plans changes.
  They are recorded nowhere yet; the maintainer was told in conversation, and after three rounds finding their cause, or
  at least recording them, is a follow-up.
- The whole browser suite on Chromium and WebKit, on ten Tensorlake sandboxes at the maintainer's request (cloud virtual
  machines; each booted the usual three test stacks and ran its share of the suite's projects through the recorder's
  generic mode, `python3 scripts/record-test-run.py --kind development ...` running
  `npx playwright test --project=<engine>-<spec> ... --retries=0`, with the binaries and web bundle built locally from
  this stack before launch-progress's landing fix; Node 24.16 there, against 24.20 locally; the sandboxes' ssh to
  themselves worked, with a fix to their setup that lands separately), runs `268ae178-8cf1-49e9-bc0e-0185f774a4f7`,
  `fc0161c2-634f-412d-91b3-edae353e7a45`, `e57f9fe2-36de-43f7-aade-46eadb89f6ee`,
  `a865423e-2157-473d-bd78-5bafcce7811e`, `85787359-a26b-4ba5-886b-5db26f29c782`,
  `d09a94d4-9ba3-4ffe-852d-a1f9e540e9bc`, `0a181417-63c4-4824-a8cb-6346a34bc6c2`,
  `905592e5-268b-477d-807f-d8ea2dd8225f`, `f88dbc3b-0bd0-42bb-b708-9d04a42e31a1`,
  `0a8e20e1-eaac-43c9-a9d9-b7a27ee49d3b`: 2891 passed, 18 skipped by design, 3 failed. Then the browser suite's
  host-terminal run (every stack's terminals in terminal hosts, `FARHELM_E2E_SUITE_TERMINALS=host`) the same way, runs
  `7f6083bf-d51d-442c-88e1-5b70d51e3309`, `917531e9-bc85-48de-87c7-d92f0a5b42b4`,
  `89a737f6-3967-42aa-a519-a90238061970`, `76be9ed9-983d-442a-a852-38bfdfad7051`,
  `cda6b941-c943-4e8e-b96d-294bf94ae240`, `d0d688aa-0374-4cd4-b816-88b95b5b3234`,
  `0a773593-de91-4798-898a-bfd970f183ed`, `60cb802f-838e-4d99-95fd-b2642908dae4`,
  `57d262a1-fe43-4398-8ff0-53f5c9acbd4c`, `a4c9e736-ed83-48cc-8198-6b43fa19b22f`: 2050 passed, 75 skipped by the specs
  themselves on terminal hosts, 5 failed.
- Each failure re-ran three times on the same build, also on sandboxes (runs `de7f9716-ee9c-48b5-8577-9b97467bd32f`,
  `e6e3b26c-8418-4e4a-9cfc-c3c976c4c8bf`, and in the host-terminal run `df17440f-34ee-4932-89b3-6519ae4ef4b5`,
  `e049551b-554f-4faa-a60a-076f30d9358e`, `32c0bae4-b4da-457c-b221-e9111ffd593f`,
  `5988d115-8b2b-45b5-a1f1-6570ec14cc8c`, `816213fe-60b3-494f-b8f3-2316945399da`). Passed 3 of 3, each a single failure
  not recorded as a flake: "create-intent-key-rebinds-on-host-change" (WebKit, multi-host: a radio button did not take
  its value), "an old session's band hides until the next visit" (Chromium, a test route's response was already
  disposed), "an idle session already seen uses its row's status foreground" (WebKit; its status word came out white),
  and "header controls stay interactive and text stays selectable" (Chromium; double-clicking the version selected
  nothing). "Post-clone commands with carriage returns refuse edits but can be cleared" (WebKit) is recorded as a flake
  by an open draft PR (#2549) and failed once more in the three; that evidence goes to its record when it lands.
- "Trash header and global confirmation remain reachable" (Chromium; after PageDown from the Delete all confirmation's
  Cancel, its scroll area stays at the top) failed at both widths in the full suite and at width 360 in the
  host-terminal run, then once in nine re-runs (six over both widths, run `e6e3b26c`, one failure at width 1000; three
  at width 360 in the host-terminal run, `32c0bae4`, all passed). Twenty runs each on sandboxes (ten per width): main
  `5c5af756` without this round's plans passed all (run `7ce0fa35-3abc-400d-ab8d-5fc457bb0731`); main with
  vs16-emoji-width and launch-progress failed 2, both at width 1000 (run `d30273b9-fc29-4e19-b4c0-036465123b34`); the
  whole stack with launch-progress's landing fix failed 3, all at width 1000 (run
  `03eda759-efdb-41ff-956b-564f134ad83d`). Those counts point at launch-progress without settling it: in the previous
  round, on a stack without launch-progress whose code is now main, the same check failed once in a valid pass (width
  360; that round's notes called it a cleanup failure, which came after this check failed) and three more times in a
  pass whose ssh-reached second host never connected, which that round discarded. So the test can fail without
  launch-progress, but whether launch-progress makes it fail more often is not shown either way, and its cause is
  unknown. The landing judged it not a reason to hold the plan; the test is being recorded as a latent flake with these
  counts after this round, and checking whether launch-progress's waiting status (its once-a-second clock or its pill)
  affects the confirmation's scrolling is a follow-up.
- launch-progress's own specs after its landing fix, on both engines and in both the ordinary and host-terminal runs
  (runs `a7cfb2f2-53ee-462f-8aae-14a6de7f04d1`, `b28405ed-9d16-47c2-a2ce-ee1c8d1c0187`,
  `215b38cf-56ba-4c89-ad57-cce8bd5d3390`, `4d14e3a3-6f3e-477c-9ab6-f3cc5080787e`): all 56 passed.
- Reused: nothing. Not run: the doctests (no doc examples changed), the desktop smoke tests (no desktop-specific code
  changed) and the native Mac app (not runnable on Linux).

The executor's report names its commit before the landing's rebase and calls the PR a draft; what landed is that commit
rebased onto current main. Nothing else in it became untrue at landing.
