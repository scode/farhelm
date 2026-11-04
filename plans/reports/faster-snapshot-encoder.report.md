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
