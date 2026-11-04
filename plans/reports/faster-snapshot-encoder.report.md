# Shorter pauses while terminal history is saved

## What this was about

Saving a grid terminal's checkpoint makes its runtime wait, so typing, resizes and requests pause until encoding
finishes. At full scrollback that was particularly slow for colorful output, and could also let a connected terminal
fall behind its host's retained output and lose earlier history. The agreed work was to make encoding faster while
preserving every snapshot byte, the choice of titles and history kept, cancellation between rows, and the existing
memory limits.

The stack removes an identical repeated measurement, keeps the first encoding when it fits instead of encoding it twice,
and reuses formatting storage for color, cursor and link changes. It adds a repeatable benchmark and four frozen byte
fixtures captured with the original encoder. All three optimization candidates are included. The final PR updates the
performance and history-loss estimates and removes the addressed TODO entry.

## Things you should know

These are release-build encoding wall times, not measured keystroke latency or CPU costs. The benchmark uses 200
columns, 60 screen rows, 12,000 history lines and a 16 MiB snapshot limit. It runs one untimed warmup and five timed
encodes per fixture, sequentially, on the same AMD EPYC Processor (with IBPB) KVM guest with 18 reported cores and 18
vCPUs. Each cell below is a median in seconds, followed by the minimum–maximum range.

| Encoder stage                   | Typical output      | True color          | Image-like output   |
| ------------------------------- | ------------------- | ------------------- | ------------------- |
| Original                        | 0.269 (0.233–0.309) | 1.568 (1.380–1.950) | 9.333 (8.987–9.857) |
| Reuse the identical measurement | 0.280 (0.263–0.298) | 1.500 (1.433–1.539) | 5.168 (5.098–5.517) |
| Keep while measuring            | 0.138 (0.120–0.151) | 0.820 (0.749–0.838) | 4.402 (4.280–5.366) |
| Reuse formatting storage        | 0.126 (0.118–0.163) | 0.424 (0.406–0.498) | 2.235 (1.915–2.348) |

The overall medians improve by about 2.13×, 3.70× and 4.18×. Shared-machine load varied. The small fitting-case changes
after measurement reuse, the smaller image-like change after keeping while measuring, and the final ordinary-output
change overlap the preceding ranges; they are not independent evidence of small speedups. The large overall reductions
are separated from the original ranges. No other build or test from this executor ran alongside the final comparisons.
All stages produced the same 3,726,399 / 12,939,296 / 16,772,035 byte blobs and dropped 0 / 0 / 9,627 history lines.

For the image-like fixture, the default 32 MiB journal and 8 MiB checkpoint spacing give a rough peak retained tail of
`8 MiB + output_rate * (encode + catch-up)`. The final 2.235 s encode alone gives about 10.235 MiB at 1 MiB/s, and uses
the remaining 24 MiB at about 11 MiB/s. Catch-up was not measured; its cost and machine load can make history loss
possible at lower rates. This improves the 1 MiB/s case without promising it is gap-free. At 100 KiB/s the 8 MiB spacing
fills in about 82 seconds; the old claim about a percentage of CPU time was removed because wall time cannot establish
that percentage.

The first buffer-allocation policy exceeded the unchanged 36 MiB transient-memory bound: the fixup-heavy case measured
38.3 and 37.8 MiB, and a private diagnostic attempt measured 37.7 MiB. An independent review also found repeated
reallocation. That policy was replaced with simpler, bounded reservations for each used buffer, while one shared logical
byte limit still decides when both buffers must be freed. The same memory case then passed at 31.8 MiB; the final
formatting change passed it at 31.9 MiB. Bounds and fixtures were never loosened. The failed runs remain distinct,
retained development observations, not latent-flake entries.

Stopping measurement as soon as it exceeds the limit was omitted: the existing selection of titles and oldest history
needs the complete costs. Each traversal still checks cancellation every 64 rows. On the fixed 500-line test fixture,
cancel-check counts changed from 16 fitting / 20 trimmed, to 16 / 12 after measurement reuse, then 8 / 10 after keeping
while measuring. The exhaustive test still cancels at every possible check.

The separate orderly-stop scheduling plan owns its three old timing quotations: the supervisor's stop module, its stop
test, and the implementation specification's "Across supervisor restarts" aside. Those locations and its TODO entry were
left alone as agreed. This stack changes neither stop scheduling nor the runtime's decision to wait for encoding.

## Open questions and possible follow-ups

There is no unresolved decision blocking this stack. Catch-up cost and end-to-end typing latency remain unmeasured; they
would be separate performance investigations. Painting directly into the final snapshot buffer and caching encoded
history remain the existing separate TODOs and were not started. No vendor code, snapshot format, dependency or recovery
policy changes are included.

## The PRs

- [#2948](https://github.com/scode/farhelm/pull/2948/changes): freeze title trimming, limited Restart seeds and
  style-removal bytes before optimizing.
- [#2951](https://github.com/scode/farhelm/pull/2951/changes): add the ignored, sequential encoder benchmark and record
  its original baseline.
- [#2953](https://github.com/scode/farhelm/pull/2953/changes): reuse the identical full measurement when the encoder
  starts trimming history.
- [#2960](https://github.com/scode/farhelm/pull/2960/changes): keep a fitting first encoding within the existing size
  and memory limits.
- [#2964](https://github.com/scode/farhelm/pull/2964/changes): reuse formatting storage for repeated color, cursor and
  hyperlink changes.
- [#2968](https://github.com/scode/farhelm/pull/2968/changes): update measured costs and conservative gap estimates, and
  remove the source TODO.

All six are open drafts in one linear stack, with one commit per PR. They have not been marked ready or merged by this
executor. Final stack verification found every published head and base in sync. Main through `0177b2d7` was audited:
other landed changes concern browser hyperlink handling, grid documentation and unrelated bookkeeping, with no encoder,
vendor, dependency or cancellation interaction. No semantic rebase was needed.

## Checks run, reused and skipped

Every runtime command ran through `python3 scripts/record-test-run.py`, with pinned nextest 0.9.148, no tmux, and zero
retries. Controlled nextest commands used four slots; the generic memory and benchmark commands ran one test process at
a time. Final records have complete source identity and untruncated retained output, with no runtime `SKIPPED` substrate
messages. Ignored/manual tests excluded by selection are not counted as executed. A portable evidence summary was
produced for the explicitly selected records; this is not a claim about all runs on the machine.

- `cargo nextest run -p farhelm-terminal-emulator`: PR1's 111 tests passed, run `943517b6-1afd-4520-a81f-ad189cf82434`.
  Reused for PR2 because it only adds an isolated ignored benchmark and inventory prose. PR3's 111 passed in
  `20d03b47-0aa0-4520-b6da-749b1a7b9e97`. Final PR4's 114 passed in `a7b922d8-50a5-452b-8b56-595b882fe18d`; final PR5's
  114 passed in `fb0db81a-a92e-4763-8296-b50fc298bc77`. This covers the old and new frozen blobs, restore/round-trip,
  size ladder, seeds, cancellation and new buffer-boundary tests.
- `cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'test(short_scripts_round_trip_at_review_size) | test(/^full_size_round_trip_seed_/)'`:
  nine tests passed on PR4 (`99d1463f-d632-41cf-b78f-5ef51242d760`) and PR5 (`ef2c64a0-57c8-400b-8b3d-8d41e96bc4a5`),
  including 300,000 short scripts and eight full-size synthetic recordings. These cover substantially more painter
  states than the frozen fixtures.
- `cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'binary(memory)' --no-capture`: all
  seven unchanged memory cases passed on final PR4 in `aa58bf1c-0a56-4b41-9bc3-e4f0a5ba7925`. PR5 reran the four
  allocation-sensitive cases using
  `binary(memory) & (test(=typical_output_at_full_scrollback) | test(=a_snapshot_of_fixups_stays_within_its_transient) | test(=a_new_maximal_hyperlink_on_every_cell) | test(=combining_marks_and_underline_colors_on_every_cell))`;
  all passed in `43b08853-13a8-425c-9aef-eaf03fe7a470`, at 7.0 / 31.9 / 31.4 / 32.4 MiB snapshot growth respectively.
  PR4's other three results are reused because grid, title and restore allocation behavior did not change in PR5.
- `cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'binary(encode_speed)' --no-capture`:
  passed before optimization in `38227a70-827e-4ad0-bab1-5405ba35f6b8`, after PR3 in
  `23fffae3-f20a-499c-849f-62d5f46ca7c8`, after PR4 in `1d0062c1-e3c1-4525-9f71-0282cfe98783`, and after PR5 in
  `41c50c6d-c3fb-4a09-a64b-a53899c02117`. Each stage's unchanged final measurement is reused as the next stage's before
  baseline. Fixture shape and repeated bytes are asserted; elapsed time is not. An earlier loaded calibration
  (`a100ca9a-339d-4782-8259-559041dbd534`) passed but was excluded from improvement claims.
- Golden premise and fixture generation passed in `77119c91-4ae5-4240-b49d-1795f0372741` and
  `513a4398-104e-4f9e-93c7-f919a8f1390a`. The original premise failed in `c4b892bf-561a-42c0-aa3a-d332689e182a` because
  it inspected the alternate screen instead of primary history; the test was corrected before recording any fixture.
  Focused PR3 and PR4 boundary/cancellation runs also passed before their full selections. The discarded PR4 allocation
  policy's failed memory observations are retained as `ad8eb215-4f9d-4bd2-9485-42c6edd04345`,
  `4a9e9f36-f70d-455a-9943-3f11995624b4` and `9e7154b9-ea78-493b-8479-03d520ca0264`; its replacement's narrow pass is
  `f1d21a33-a783-45a9-84c3-a612dea8634d`. Failures are not included in pass evidence.
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-terminal-emulator --all-targets -- -D warnings`, and
  `<isolated-python> -B scripts/check-test-sleeps.py` passed for the code/test units; the sleep checker inspected 397
  delays with zero missing rationales. `python3 releasing/check-changelog.py format` passed for the fragment.
  `python3 scripts/check-agents-md.py` passed after the inventory edits, with zero errors and three pre-existing
  warnings.
- PR6 ran `dprint check SPEC.md SPEC_impl.md TODO.md`, `cargo fmt --all -- --check` and `git diff --check`; all passed.
  No Rust, JS, browser or other runtime test was run for its prose-only changes. Per-command PR5 evidence at `08286ce2`
  remains applicable at final `d8e50da5`, which changes only prose and removes the source TODO. Dprint has no applicable
  plugin for the touched Rust and fixture/fragment files; an earlier no-files exit was not claimed as a pass.
- Supervisor runtime integration, browser/desktop, emulator-vs-tmux, vendor, installer and full-workspace suites were
  skipped: the encoder's output and public API are unchanged, with byte, restore, cancellation, broad round-trip and
  memory evidence above; none of those components changed. Hosted CI was not dispatched. No live installation was used.

## The review gate's outcome

Each of the six PRs received the required single fresh-context gpt-6.1-sol review at high effort. The
keep-while-measuring review found repeated cross-buffer reallocations in the first policy; that finding was accepted. A
fresh same-model scope assessment supported removing the unnecessary capacity-sharing machinery, and the same PR
reviewer found the replacement resolved the issue with no new findings. All other PR reviews reported no findings.
Reviews were static; runtime evidence comes from the executor's recorded commands above.

Every commit/PR message passed a separate fresh-context wording check. One documentation-message reader accidentally
opened the diff before its blind phase; it was replaced with a fresh reader before the message was used. A separate
fresh-context report reader checked this report for sufficient product context and public-repository hygiene before
delivery. No required behavior was omitted and no agreed guarantee was weakened.

### Landing

Landed with no fixes of its own, first of the four plans landed together in this round (with colored-harness-marks,
discord-link-in-app and grid-drop-and-drag-notice, stacked above it). Saving a new terminal's checkpoint, which pauses
its typing, resizes and requests until it finishes, now takes about half the time for typical output and about a quarter
for very colorful output at full scrollback, with the saved bytes unchanged.

- #2948: frozen byte fixtures captured with the original encoder.
- #2951: a repeatable benchmark of the encoder.
- #2953: no second identical measurement of an oversized snapshot.
- #2960: keeping the first encoding when it fits instead of encoding twice.
- #2964: reusing formatting storage for color, cursor and link changes.
- #2968: the specification's timing and history-loss estimates, and the removed TODO entry.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the grid terminal's link fix (#2949), recovery of unavailable terminal hosts and tabs
(#2950, #2956), the orderly stop's checkpoint order, core cap and skipping of small tails (#2952, #2954), and PRs that
only added TODO entries and plans. None touches the terminal emulator, so the encoder change and the frozen fixtures are
as the executor tested them. A fresh-context review checked the encoder against what those changes rely on and found no
interaction: its interface is unchanged, it still checks for cancellation every 64 rows (the orderly stop cancels
encodes that way), and the supervisor's stop and recovery tests hold encodes before the encoder runs, so a faster
encoder cannot change what they test.

The rebase conflicted in TODO.md, where main had removed and added entries near the one this plan removes, and in the
implementation notes' paragraph on checkpoint cost, which this plan and the orderly-stop plan (#2954) both rewrote in
different sentences. The landing kept this plan's new timing and history-loss sentences followed by #2954's three
sentences on the orderly stop; a check confirmed each commit changes exactly the sentences it changed when tested.

#### Things you should know

- The implementation notes and a supervisor comment name the benchmark machine's processor model, more specifically than
  this repository usually describes a test machine ("an 18-vCPU x86-64 Linux virtual machine" would do).
- The executor's release-mode measurements (memory, the full round trip, encode speed) were not repeated: nothing that
  landed since touched the terminal emulator.

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
