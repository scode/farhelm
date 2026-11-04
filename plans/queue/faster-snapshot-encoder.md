# Grid terminals: shorter typing pauses while a checkpoint is saved

Written against main at 856099b8 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Typing into a grid terminal pauses while its checkpoint is encoded, longer than it needs to.** The supervisor
> checkpoints a terminal in a terminal host (encodes its emulator as a snapshot and hands it to the host) after enough
> output and at every orderly stop, and a Restart seeds the new host with the same encoder. While a checkpoint encodes,
> the terminal's runtime does nothing else, so keystrokes, resizes and requests wait: at full scrollback about 0.3 s for
> typical output, 1.6 to 1.9 s in true color, and over 10 s for a scrollback colored cell by cell like an image
> (SPEC_impl.md, "Recovery and checkpoints", release build). Such an image-like scrollback receiving about 1 MiB/s
> cannot be checkpointed as fast as it fills the host's journal, so it may gap while connected ("Some earlier output
> could not be recovered"); SPEC_impl.md names a faster encoder as the remedy, and the stop's checkpoints (the entry
> about the burst of checkpoints at an orderly stop) get cheaper with it. Part of the cost is the size-limit logic in
> `crates/farhelm-terminal-emulator/src/snapshot/mod.rs` (`Emulator::snapshot_until`, `fit_titles`, `fit_history`). So
> that the supervisor never holds a stream larger than the 16 MiB limit, it first paints the whole state in
> `Mode::Measure` (counting bytes and dropping them, `snapshot/v1/paint.rs`) and then paints it again in `Mode::Keep`
> with buffers reserved to the measured size, so every checkpoint that fits paints the terminal twice. One that does not
> fit and has no title stack is measured in full by step 1 (`fit_titles`) and again, identically, by step 2's first
> measurement (`fit_history` at the same `min_skip`, with all of zero titles left out), then measured once more after
> dropping the oldest lines, and then kept: a scratch benchmark during the terminal-host work (200x60, 12,000 lines of
> history, single runs on a loaded machine, about 30% noise) painted about 188 MiB to produce a 16 MiB blob, where an
> unlimited encode of the same 78.7 MiB state took 9.7 s. A measurement also runs to the end after it has passed
> `max_bytes`, and the painter allocates on every pen change (`pen_changes` and `pen_reset` build a `Vec<String>`,
> joined through `format!`). Candidates, to be profiled first: reuse step 1's measurement in step 2 when the two would
> be identical; keep what is painted while measuring as long as it stays under the limit, so that a snapshot that fits
> is painted once; stop measuring past the limit where nothing needs the total (the excess decides how many titles or
> lines to drop, so check what the ladder still reads); and drop the per-pen-change allocations. Constraints: the
> encoder keeps writing exactly the bytes it writes now (SPEC_impl.md, "Terminal snapshots": a shipped format never
> changes, checked byte for byte by `crates/farhelm-terminal-emulator/tests/snapshot_golden.rs` against
> `tests/fixtures/snapshots/v1/`); the supervisor never holds a stream past the limit, nor a `Vec` that grew by doubling
> past it (`Mode::Keep`'s docs); the encoder still looks at its `Cancel` between rows, which is what keeps an orderly
> stop bounded; and the size ladder's guarantees in the module docs ("Size") hold. The repository has no encoder
> benchmark; add one with the change so the speed-up is measured rather than assumed.

### What is known, and what is not

NOTE: Nothing here was profiled. The planning research read the code; it ran no benchmark. Every speed figure below is
either quoted from the repository or an estimate, and says which. Line numbers are approximate; correct them to the
truth as you go.

Here's the TLDR of the encoder as it stands, all under `crates/farhelm-terminal-emulator/src/snapshot/`:

- **The ladder,** `Emulator::snapshot_until` (`mod.rs` around 417-493), in three steps that match the module docs'
  "Size":
  1. `fit_titles` (around 797-834) at `Cells { skip_history: min_skip }`: measure (around 804); if it fits, keep (around
     810); if not, drop the fewest titles whose `title_costs` cover the excess and measure again. It returns `None` only
     after a measurement with every title dropped (around 816-817).
  2. `fit_history` (around 845-886), titles all dropped: measure (around 853); if it does not fit, use `history_costs`
     (recorded per row, `v1/paint.rs` around 1106-1108) to drop the fewest lines covering the excess and measure again
     (the carried pen can shift a few bytes); keep once it fits (around 859).
  3. `Content::Nothing` through `fit_titles`, else a last-resort `Mode::KEEP` with no measurement.

  The Restart seeds (`history_seed_until` around 523-551, `history_seed_from_lines_until` around 735-789) share
  `fit_history` and have no step 1.
- **Modes do not change the bytes.** `v1::encode` (`v1/mod.rs` around 209-240) calls `paint::paint` (`v1/paint.rs`
  around 165-411) and then `container` (`v1/mod.rs` around 264-356). What the painter writes does not depend on `Mode`
  (`v1/paint.rs` around 110-134); the mode only decides whether buffers are kept (`drop_measured` around 1078-1083,
  `fix_cell` around 1062-1075, `finish` around 1046-1060). That is what lets every change below stay byte-identical. A
  measurement still formats every byte into a buffer that starts at 64 KiB (around 1018) and is cleared once per row; a
  row that paints more than that (a wide row of cells each with a different long hyperlink, say) grows it by doubling.
  `container` copies stream and fixups into a blob reserved exactly to its length (around 315-347), so the transient
  peaks at about twice the blob.
- **The duplicated measurement is more general than the entry says.** Step 2's first call
  (`with_history(min_skip, Measure)`, `mod.rs` around 451-463) encodes `Cells { min_skip }`, all titles dropped and
  `trimmed_for(min_skip > 0)`, which are exactly the inputs of step 1's final measurement. So whenever step 2 runs at
  all, with or without a title stack, its first measurement repeats step 1's last one byte for byte. The entry's 188 MiB
  fits four paints: 78.7 + 78.7 + about 16 (the re-measure) + about 16 (the keep). Extrapolating from its 9.7 s for one
  paint, the limited encode of that state takes about 23 s; that is arithmetic on a noisy scratch number, not a
  measurement.
- **Stopping a measurement past the limit does not work.** The ladder needs the total: `fit_titles` uses
  `measured - max_bytes` to choose how many titles to drop (around 822), and `fit_history` uses the excess and the full
  `history_costs` to choose the cut (around 857, 877-883). The pen carries from row to row, so nothing can be painted
  newest-first. The one safe early exit (a step 1 round with titles left, once `written` passes `max_bytes` plus the
  remaining titles' costs) helps only a grid far over the limit that also has a title stack, which is rare.
- **Allocations per pen change are worse than the entry says.** `Writer::set_pen` (`v1/paint.rs` around 874-888) builds
  `pen_changes` (around 706-756, one `String` per parameter, colors through `format!`) and usually `pen_reset` (around
  773-783), `join`s both only to compare lengths (keeping the reset when strictly shorter, around 881), then `join`s
  again inside `format!` (around 886): about ten heap allocations for a true-color foreground and background change.
  `cup`, `ech`, `set_link` and `raw_cup` also go through `format!`. `sgr_setting` (around 762-769) serves the tmux
  seed's carried lines, not the blob. The `Arc` clone per linked cell is in vendored code, so it stays.
- **Cancel looks** happen every 64 rows (`ROWS_PER_CANCEL_CHECK`, `v1/paint.rs` around 65; the check around 1101), in
  keeping and measuring alike. `Cancel::after_looks` (`mod.rs` around 346-351, test-only) counts them.

**Estimates, unprofiled guesses to check against the benchmark:** for typical output, keeping while measuring is about a
2x win (0.3 s to about 0.15 s), because the per-cell work dominates and pen changes are rare. For true color (about 5
bytes per cell, a pen change every few cells), removing the pen allocations and painting once could combine to 3-6x. For
image-like over the limit, the allocations probably dominate each paint; reuse and keep-while-measuring cut the bytes
painted from about 188 to about 95 MiB, and if the allocation work gives another 2-3x, the limited encode might drop
from about 23 s to 4-6 s. Treat all of this as a guess about where to look, not a target.

**The image-like gap, in arithmetic.** A full image-like snapshot is capped at 16 MiB, so automatic checkpoints come due
every `min(4 x 16 MiB, journal cap / 4) = 8 MiB` of output (`crates/farhelm-supervisor/src/hosts/terminal/checkpoint.rs`
around 62-76). While an encode runs the runtime reads nothing, so a rough steady-state peak tail is
`8 MiB + rate x (encode + catch-up)`, and a gap needs that past the 32 MiB journal cap. At 1 MiB/s with a 23 s encode it
gaps; at 4-6 s it peaks around 15 MiB. At a few MiB/s it would gap again whatever the encoder does, which is why
Decisions say "improved and measured", not "fixed".

**What guards correctness today** (all under `crates/farhelm-terminal-emulator/`):

- `tests/snapshot_golden.rs` (`scripts_still_encode_to_their_golden_blobs` around 493, and the restore and feature
  checks beside it) over 25 scripts in `tests/fixtures/snapshots/v1/`. Only `trimmed-history` and `screens-dropped`
  (around 231-250) exercise the ladder; no golden drops titles or combines titles with history. The harness's `Take`
  (around 59-66: `Full`, `Limited(usize)`, `Seed`) cannot express a seed under a limit, and
  `scripts_exercise_their_feature` ends in a `_ =>` arm asserting no history was dropped (around 647). The `styles`
  script already sets every flag, every underline style, and indexed and RGB colors with resets between. The ignored
  `write_missing_golden_fixtures` (around 658) writes a new script's fixtures and never overwrites one: fixtures are
  added to, never regenerated (`src/snapshot/mod.rs` around 25, SPEC_impl.md around 2558).
- `tests/snapshot.rs`: the ladder tests (around 730, 801), trimmed-mark tests (around 780, 1177), the cancel test
  (around 650), seed tests (around 923-1242), and the ignored `short_scripts_round_trip_at_review_size` (around 1629).
  `tests/snapshot_roundtrip.rs`: the proptest `generated_output_round_trips` and the ignored full-size runs.
- `snapshot::cancel_tests` (`src/snapshot/mod.rs` around 888-1014). `a_cancel_landing_at_any_look_stops_the_encode`
  already counts uncancelled looks at 16 MiB and at 4096 bytes, and asserts `looks > 2 * rows_per_paint` at 4096 as a
  premise ("the limit makes the encoder measure again", around 942-947: today about 8 + 8 + 2 + 2 looks against more
  than 16). Reusing step 1's measurement breaks the premise (about 12 looks), not the property. `ceil(rows / 64)` looks
  holds per `paint_rows` call; `paint` calls it twice for an alternate-screen state, so exact counts want a
  primary-screen fixture such as `with_history`.
- `tests/memory.rs` (ignored, Linux, release): `SNAPSHOT_PEAK_MIB = 36` (around 48) and
  `a_snapshot_of_fixups_stays_within_its_transient` (around 352-374) judge the transient. It also prints an encode time
  at 200x60 with 12,000 lines, from a line generator (around 201-209).

### Acceptance criteria

- **Same bytes.** For every input, the encoder writes exactly the bytes it writes at 856099b8: every existing golden
  passes unchanged, and so do the new goldens this plan records with today's encoder before changing it.
- **Constraints kept:** the supervisor never holds a stream past `max_bytes`, nor a buffer whose capacity grew past it;
  the encoder looks at its `Cancel` between rows whether it keeps or measures; the module docs' "Size" guarantees hold;
  `tests/memory.rs` passes at its current bounds.
- **Measured.** An encoder benchmark exists, lands before any optimization, and every perf PR and the report give
  before/after medians for each fixture from the same machine. The cancel test's look counts pin how many paints a
  fitting and an over-limit snapshot take.
- **The candidates, each landed or left out with evidence:** reuse step 1's last measurement in step 2; one paint for a
  snapshot that fits; no heap allocation per pen change, link change or cursor move once the writer's buffers have
  grown.
- **Figures updated; the gap restated, not erased.** This plan owns the encode figures. SPEC_impl.md "Recovery and
  checkpoints" (around 1943-1957) and the module docs of `crates/farhelm-supervisor/src/hosts/terminal.rs` (around
  215-217, 248-253) carry the measured before and after times, with the method (the benchmark, release build, median of
  N) and the machine by class (CPU model and core count, never a host name). They, and SPEC.md (around 2143-2145, "about
  a megabyte a second"), give the rate at which a gap becomes possible if the measurement moves it, and still say a gap
  is possible. The report gives the image-like limited encode time with the arithmetic above.
- **Changelog and TODO:** a changelog fragment per root `AGENTS.md`, "Changelog fragments". The last PR removes the
  TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-11-03, on the planning page:

- **Output stays byte-identical.** A shipped snapshot format never changes (the entry's constraint, SPEC_impl.md
  "Terminal snapshots"). No format version 2, no approximate cut that changes which lines are dropped.
- **The image-like 1 MiB/s gap is improved and measured, not guaranteed fixed.** Whether it gaps depends on the output
  rate, so no encoder speed can promise it never happens.
- **Out of scope: painting the snapshot straight into its final buffer** (removing `container`'s copy). The planning PR
  adds a separate Near term TODO entry for it. Do not start it.
- **Out of scope: reusing already-encoded scrollback between snapshots** (an incremental or history cache). The
  maintainer chose neither design; the planning PR adds a Near term TODO entry to revisit how snapshots work in general
  and have the maintainer take a position on incremental maintenance. Do not start that work.
- **Fence: no change under `vendor/`.** Code changes stay in Farhelm's own `crates/farhelm-terminal-emulator`, almost
  all of it in `src/snapshot/`, plus its tests and crate docs. Outside that crate this plan changes only prose that
  quotes encode figures or the gap rate (listed under Acceptance criteria) and the validation inventory.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort for every PR (see Review gate). If it cannot
  be reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:**

- SPEC_impl.md: "Terminal snapshots" (around 2549-2597: the format never changes, goldens added to and never
  regenerated, the memory budget's "about 32 MiB while a snapshot is taken"), "Recovery and checkpoints" (around
  1901-1960). SPEC.md around 2143-2145 (gaps while a supervisor runs).
- `crates/farhelm-terminal-emulator/src/snapshot/mod.rs` module docs ("Size", around 44-67), `src/lib.rs` "Memory
  budget" (around 75-80), and `Mode`'s docs in `src/snapshot/v1/paint.rs`.
- Root `AGENTS.md`: "Sharing the machine with other agents", "Tests and validation", "Conventional Commits", "Changelog
  fragments", "TODO.md and FLAKES.md" (add no TODO entries of your own).
- `.agents/test-authoring.md`, `.agents/narrow-tests.md`, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The benchmark is an ignored nextest test binary in the style of `tests/memory.rs`, with no new dependency (no
  criterion or divan): `tests/encode_speed.rs`, run as
  `cargo nextest run -p farhelm-terminal-emulator --release --run-ignored only -E 'binary(encode_speed)' --no-capture`.
  Fixtures at 200x60 with 12,000 lines of history, built by feeding generated output: typical (a copy of `memory.rs`'s
  eight-line generator, not moved into `tests/common/`, which every test binary shares); true color (runs of a few
  cells, each with a new 24-bit foreground and now and then background, calibrated near the documented 12.6 MB blob);
  image-like (every cell a new 24-bit background and foreground, over the 16 MiB limit, so it runs the ladder). For
  each, print the median and spread of about five runs of `snapshot(16 MiB)`, the blob size, and history lines dropped.
  Wall time is never asserted. Expect the image-like pair to take minutes (about 23 s an encode today, if the estimate
  holds).
- No separate paint-count test. PR 3 turns the cancel test's premise into exact look counts for the fitting limit and
  the over-limit one, on a primary-screen fixture, and PR 4 updates them (after the plan: one paint's looks for a
  snapshot that fits, one full paint plus one paint of the kept rows over the limit). PR 2's description records today's
  counts as the baseline. Low priority: if it fights you, keep the premise as an inequality and log why.
- Drop "stop measuring past the limit" entirely, for the reasons above, and say so in the report.
- Keep-while-measuring: a single byte budget replaces `Mode`, not a third variant beside it. The painter keeps stream
  and cell fixups while the blob they would make stays within the budget (roughly
  `written + cell_fixup_bytes + HEADER_BYTES + 9`, the two counts and the smallest REP field; verify), and past it frees
  both buffers and carries on measuring, so `stream_len`, `cell_fixup_bytes` and `history_costs` come out as a
  measurement's would. The last resorts (`mod.rs` around 480 and 541) pass `usize::MAX`. `Mode::Keep`'s measured sizes
  and `Encoded::keep` (`v1/mod.rs` around 197-202) lose their callers and go; "still keeping" becomes a run-time flag in
  the `Painter`. `container` decides on the exact length: a kept paint within `max_bytes` becomes the blob, otherwise it
  returns a measurement (cursor fixups, known only at the end, can still push it over). `fit_titles` and `fit_history`
  skip their separate keep when the paint was kept.
- The keep-to-measure switch is checked at every write, not once per row, because one row can paint far more than 64 KiB
  and would otherwise grow a buffer past the budget: the stream's choke points are `Writer::raw` and `Writer::print`,
  and the fixups' is `Painter::fix_cell` (which already knows a record's size before writing it). With that in place,
  capped doubling (`reserve_exact` up to `min(2 * capacity, budget)`) and reserving the budget up front both keep
  capacity within `max_bytes`; pick one, log it, and let `tests/memory.rs` judge.
- New goldens, recorded by today's encoder before any refactoring: (i) a title stack partly dropped; (ii) every title
  dropped and then history trimmed; (iii) a Restart seed trimmed by a limit, which needs a new limited-seed `Take`
  variant. (ii) and (iii) drop history, so each needs a named arm in `scripts_exercise_their_feature` asserting its
  feature (for example every title dropped and `dropped_history_lines > 0`) rather than the `_ =>` arm. (iv), a line of
  SGR transitions, only if it adds what `styles` lacks: the removal codes 22, 23, 24, 27, 28, 29, 39, 49 and 59, and an
  exact length tie between reset and changes. Check that by hand against `styles`.
- Pen allocations: give `Writer` two reusable byte buffers. `pen_changes` and `pen_reset` write their `;`-separated
  parameters into them (`write!` for the numbers), and `set_pen` compares the two real lengths with the same strict `<`
  and appends `ESC [`, the shorter one, and `m`. The tokens, order, separators and tie-break stay the same code, so byte
  identity holds by construction; no arithmetic length computation. `sgr_setting` calls the same function into a fresh
  `String`, so no second copy survives. No committed oracle or proptest: if you want extra assurance, compare the new
  code against the old once locally, uncommitted, before deleting the old code, and log the result as a DECISION.
  Hand-roll decimal output only if the benchmark shows formatting still costs after the allocations are gone. The other
  per-cell and per-row writers get the same treatment: `set_link`, `cup` (CUF, CHA, CUP), `ech` and `raw_cup`. Leave the
  once-per-snapshot writers (palette, titles, cursor style, modes, scroll region, `v1/paint.rs` around 284-375) alone.

## Outline

1. `plan/faster-snapshot-encoder/01-goldens` (`test:`): the new golden scripts and their fixtures, written by
   `write_missing_golden_fixtures` from today's encoder, with the limited-seed `Take` variant and the named feature
   arms. No changelog fragment.
2. `plan/faster-snapshot-encoder/02-benchmark` (`test:`): `tests/encode_speed.rs` and the inventory entries in
   `.agents/validation.md` (the terminal emulator selection's ignored tests) and `.agents/run-all-tests.md`. The
   baseline numbers and today's look counts go in the PR description. No fragment.
3. `plan/faster-snapshot-encoder/03-reuse-measurement` (`perf:`): `fit_titles` hands its last measurement to
   `fit_history` as an optional first measurement for `skip == min_skip`; seeds are unchanged. The cancel test's premise
   becomes exact look counts. Adds the changelog fragment (kind `changed`), which later perf PRs edit rather than add
   to.
4. `plan/faster-snapshot-encoder/04-keep-while-measuring` (`perf:`): one paint for a snapshot that fits, with a byte
   budget replacing `Mode`, the per-write switch and bounded buffer growth. Update the look counts. Rewrite the docs of
   `Mode`'s replacement, `fit_titles` and `fit_history` (`mod.rs` around 791-796, 836-844) and the crate's "Memory
   budget" ("Sizes are found by measuring first ..."). Run `tests/memory.rs`.
5. `plan/faster-snapshot-encoder/05-pen-allocations` (`perf:`): reusable buffers for `set_pen`, `set_link`, `cup`, `ech`
   and `raw_cup`.
6. `plan/faster-snapshot-encoder/06-docs` (`docs:`): the measured figures and the restated gap rate in the places under
   Acceptance criteria, and the TODO.md removal. It may fold into PR 5 if it is small.

The fragment is written for someone running Farhelm, along the lines of: typing into a busy terminal pauses for less
time while its history is saved, and very colorful scrollback is less likely to lose earlier output.

### Coordination with `stop-checkpoint-burst`

That plan changes how an orderly stop schedules checkpoints and touches only the supervisor. The two may run
concurrently and neither depends on the other. It replaces the encode figures quoted in the stop's docs
(`service/core/host_handover.rs` around 37-39, `service/core.rs` around 18535-18536, and the "(up to about two seconds
...)" aside in SPEC_impl.md "Across supervisor restarts") with references to "Recovery and checkpoints", so leave those
three places alone even if they still quote old figures when you finish; say so in the report. On a rebase conflict in
SPEC_impl.md with that plan's work, keep both sides' meaning. Leave its TODO.md entry alone.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Run runtime tests through
`python3 scripts/record-test-run.py`. Expect:

- every PR: the "terminal emulator" selection (`cargo nextest run -p farhelm-terminal-emulator`, about a minute; it
  holds the goldens, round trips, ladder, cancel and seed tests), `cargo fmt --all -- --check`, clippy on
  `farhelm-terminal-emulator` with all targets and `-D warnings`, and `python -B scripts/check-test-sleeps.py` with the
  interpreter `docs/test-sleep-check.md` prepares;
- every perf PR: the benchmark before and after, on the same machine, with the load noted (a busy shared machine is an
  environment event, not a result);
- PRs 4 and 5, by hand in release: `-E 'test(short_scripts_round_trip_at_review_size)'` and
  `-E 'test(/^full_size_round_trip_seed_/)'`, which drive the painter over far more states than the goldens;
- PR 4: `-E 'binary(memory)'` in release on Linux with `--no-capture`;
- `dprint check` on changed files, `python3 releasing/check-changelog.py format`, and
  `python3 scripts/check-agents-md.py` for the inventory changes;
- optional, once at the tip: the supervisor's "host terminal runtime" selection, which drives real encodes. The
  supervisor's API and behavior do not change, so its other selections are not needed.

## Fallbacks

A candidate that cannot be made byte-identical, or cannot keep the memory and cancel constraints, is left out, with the
evidence in the report; the agreed bar is a measured improvement, so that weakens nothing the maintainer asked for.
Never loosen a test bound (`SNAPSHOT_PEAK_MIB`, a golden) to make a candidate fit. Changed bytes, anything under
`vendor/`, and the two out-of-scope items are fenced by Decisions; no scope reassessment lifts them.

Examples of departures that need the scope reassessment:

- a mechanism beyond the outline (a cache, a new painter abstraction, a rework of the ladder);
- code changes outside `crates/farhelm-terminal-emulator`;
- a new dependency, or a benchmark harness other than an ignored test binary.

Decisions to log: each candidate's verdict with its measured effect, the buffer-growth strategy, every golden added (and
whether (iv) was), any one-off local comparison of the pen code and its result, every test changed and why, the
before/after numbers and the machine class, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-faster-snapshot-encoder-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/faster-snapshot-encoder/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). For the perf PRs, ask it specifically whether any input could now produce different bytes,
hold a buffer past `max_bytes`, or skip a cancel look. When a PR changes tests or their fixtures, include the full text
of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one
plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
