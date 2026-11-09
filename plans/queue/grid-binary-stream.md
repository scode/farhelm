# Compact grid stream and scrollback

Written against main at 9e3ab60b on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Overhaul the grid stream's encoding and the grid client's history storage.** Grid terminals (terminals in a terminal
> host) get their screen and history from the supervisor as JSON messages, and the page keeps every history line
> decoded, as one JavaScript object per run of styled text plus a style object per distinct color pair. Measured in
> WebKit (the Mac desktop app's engine) at the 12,000-line scrollback cap, 200 columns, growth in resident memory once
> settled: typical output 44 MiB (xterm.js 65), a different 256-color color in every cell 364 MiB (xterm.js 408), a
> different true color in every cell 737 MiB (xterm.js 502). The benchmark behind those numbers was never committed. On
> 2026-11-03 the maintainer asked for the wire encoding and the in-memory representation to be overhauled together so
> that neither is hideously inefficient, and judged making history compact without changing the encoding a waste of
> time. The shape discussed: one compact binary encoding for the grid stream, which the page stores history in nearly as
> it arrives, decoding only the rows on screen and the rows that selection, copying, word and link lookups touch.
> Drawing (one element per visible row) stays as it is, and compressing older history is something to measure afterwards
> rather than assume. The grid stream is not a permanent format, so a new encoding is an ordinary protocol bump
> (`crates/farhelm-proto/src/grid.rs`, "Why this shape", which chose JSON for readable traces and to avoid a
> hand-written binary decoder in the browser, the place where bounds would be easy to get wrong). Commit a benchmark
> that measures memory, attach size and attach time before and after.

What is true today, from a read of main at 9e3ab60b (line numbers approximate):

- **Definition:** `crates/farhelm-proto/src/grid.rs`: "Why this shape" around 9-33, rows and runs around 95-149, screen
  updates around 151-180, history around 182-205, ordering and the attach sequence around 240-260, flow control around
  291-317, bounds around 337-350, `encode`/`decode` (serde_json) around 1174-1190, and `GridStreamCheck` for the
  cross-message rules. Supervisor-to-page kinds: `hello`, `state`, `rows` and `frame` (with `shift`), `history`,
  `history_reset`, `clipboard`. A row is `{"w":bool,"r":[run...]}`, a run `[col, width, text, style(, link)]`, a style
  `[flags, fg, bg(, ul)]`, colors `-1`, `0..255` or `0x1000000+RGB`, links a per-message `links: [[id, uri]]` table.
  Page-to-supervisor input (`GridInput`) is JSON too. Golden fixtures are in
  `crates/farhelm-proto/grid-fixtures/{output,input}/{valid,tolerated,invalid}`, and
  `crates/farhelm-ui/js-tests/grid-proto-fixtures.test.js` must name every file there.
- **Producer:** `crates/farhelm-supervisor/src/grid_output.rs` (`row_from_cells` around 143, `SentScreen::update` around
  403, `HistoryCursor` around 594, `rows_messages`/`history_messages` around 887/913 packing to `MESSAGE_TARGET` = 32
  KiB measured in JSON bytes, around 70); encoding at `crates/farhelm-supervisor/src/service/grid_stream.rs` around 820.
  SPEC_impl.md around 2629 describes the sender.
- **Relay:** the helm passes grid messages through unread, as binary WebSocket frames
  (`crates/farhelm-helm/src/terminal.rs` around 38-59 and 828); its text frames carry only its own notices. Its queue
  limits are in bytes (`crates/farhelm-helm/src/client.rs` around 84-210, `GRID_PAUSE_BYTES` = 2 MiB), so a denser
  encoding fits more lines before a pause.
- **Consumer:** `crates/farhelm-ui/assets/terminal.js` around 4733 sets `binaryType = "arraybuffer"`; `grid-terminal.js`
  around 1974 calls `gridModel.decodeFrame`, which UTF-8-decodes and `JSON.parse`s (`grid-model.js` around 146). History
  is queued raw and drained in 8 ms slices (`drainHistory`, `grid-model.js` around 657), decoded by `decodeRow`
  (around 224) with interned styles (`internStyle` around 265), and stored in `LineStore` (around 333-386). The single
  point every history reader reaches is `GridModel.historyLine` (`grid-model.js` around 773-779): selection and copy,
  word and line lookup and links go through `modelSource(...).line(abs)` (`grid-buffer.js` around 585; links are
  `oscLinkAt`, `linkIdentities` up to 128 rows back, `rowLinks`, `urlWindow`, `linkLines`/`lineView`, feeding
  `terminal-links.js` and `terminal-files.js`), while drawing, `snapshot()` and the e2e helpers
  (`e2e/tests/helpers/term.ts` around 188 and 267, `real-agent.ts` around 153) call `historyLine` directly. So on-demand
  decoding belongs behind `historyLine`, and its callers need no change. There is no scrollback search. `history_reset`
  never carries rows.
- **Row identity:** link hover and late-answer checks compare row objects (`rowAt(abs) !== row`, `grid-terminal.js`
  around 1283-1312 and 1365-1368), which works because rows are never mutated (comment around 1255). Rows decoded on
  demand need a stable replacement for that identity.
- **Versioning:** `PROTOCOL_VERSION` = 70 with its rules at `crates/farhelm-proto/src/lib.rs` around 192-248; a
  mismatched peer is refused, and the page is gated on the helm's build stamp, so its JavaScript always matches its
  helm. The terminal host carries raw pty bytes and snapshot blobs and never sees the grid stream
  (`crates/terminal-host/SPEC.md`), so its change policy does not apply.
- **Rust readers of the stream** (`crates/farhelm/tests/e2e/harness.rs` around 178, a test in
  `crates/farhelm-helm/src/provisioning.rs` around 12689, `grid_output/test_client.rs`) all call `GridOutput::decode`,
  which can keep its signature, so they need no change. What does change: `grid_tests.rs` and the JSON size estimates in
  `grid_output.rs`.
- **JS tests feed JSON.** About 150 inline `rows`/`frame`/`history` messages across `grid-model.test.js`,
  `grid-buffer.test.js` and the grid-client specs, plus `crates/farhelm-ui/js-tests/fixtures/grid/attach.jsonl` and
  `scroll.jsonl`, are JSON. They are hand-written; no recording tooling exists.
- **Benchmarks:** nothing measures browser memory yet. Precedents: `crates/farhelm-terminal-emulator/tests/memory.rs`
  (ignored tests run on demand in release, same 12,000×200 setup); `e2e/grid-client/playwright.config.ts` loads the grid
  component on a static page fed streams in Chromium and WebKit with no Farhelm stack, which is the natural home for the
  new benchmark. JS unit tests run with `node --test` in `crates/farhelm-ui/js-tests`. The grid assets are plain scripts
  with no bundler and contain no binary decoding today. WebKit exposes no page-visible heap API; on Linux, both engines'
  content processes can be read from `/proc` (`VmRSS`, `VmHWM`), and the pinned Playwright offers `page.requestGC()`.
  The TODO's numbers came from the Mac's WebKit, a different build from Playwright's Linux WebKit, so compare main with
  the stack here rather than with them.

### Acceptance criteria

- **A committed benchmark, run first.** An opt-in tool, run on demand and not part of the normal test runs, in or beside
  the grid-client harness. It reports, for each of three streams (typical output; a different 256-color color in every
  cell; a different true color in every cell; each at 12,000 lines of scrollback and 200 columns), in WebKit and
  Chromium: growth in the content process's resident memory from a pre-attach reading once settled (`VmRSS` after a
  forced GC) and its peak while attaching (`VmHWM`, with a fresh browser per measurement), attach size (bytes the page
  received), and attach time (until history is fully drained). Document how each figure is read. The streams are
  produced through the real Rust encoder at run time, so the same tool measures main and the stack. It must not be
  picked up by normal grid-client runs (no `*.spec.ts` name in a normal project), must not poll `snapshot()` (which
  decodes all history) and must not hand the whole stream to the page in one `page.evaluate`. It is committed in the
  stack's first PR, and the numbers for main are recorded in the working log before any encoding change.
- **Binary screen and history messages.** `rows`, `frame` and `history` use one compact binary encoding defined in
  `crates/farhelm-proto/src/grid.rs` next to a rewritten "Why this shape" saying why binary now. `hello`, `state`,
  `clipboard` and every page-to-supervisor message stay JSON. Both kinds still travel in binary WebSocket frames,
  distinguished unambiguously (for example by a leading tag byte that cannot begin a JSON message). The protocol version
  bumps per `lib.rs`'s rules, with the reason in the commit and in SPEC_impl.md. The supervisor's packing target counts
  encoded bytes.
- **A hand-written JavaScript decoder**, bounds-checked on every read, refusing a malformed message the way the page
  refuses malformed JSON today. It is tested against Rust-encoded fixtures checked into the fixture tree (valid and
  malformed cases; no "tolerated" class for binary, since the stream is locked to one protocol version and needs no
  forward-compatibility leniency), each stored with a JSON description beside the binary file, and a fixed-seed corpus
  of randomly generated rows produced by Rust and checked in the same way, so JS decodes the bytes and compares with the
  description. A Rust test fails if any checked-in bytes drift from the encoder. The fixture-naming rule in
  `grid-proto-fixtures.test.js` keeps holding. The JS tests that feed JSON messages today switch to a test-only JS
  encoder in their feed helpers, itself checked against the Rust fixtures.
- **Compact history storage.** The page keeps history in a compact form close to the received bytes, with a per-line
  index, and decodes behind `GridModel.historyLine` only the rows a reader asks for (the visible rows, and the rows
  selection, copy, word and link lookups touch). Every history row is still fully validated when it is drained, as
  today, so a malformed row is refused on arrival rather than when someone scrolls to it. Row identity for the hover and
  late-answer checks keeps working through a stable per-line record or token (the reset counter plus the line number,
  say), not object identity of a re-decoded row. Dropping the oldest lines at the cap and `history_reset` release their
  memory. The screen may stay decoded.
- **The after numbers.** At the end, the benchmark runs main and the stack tip interleaved, several times each, and
  compares medians, since the machine is shared and loaded. Success: memory growth clearly lower than main in all three
  streams on both engines, attach size lower, attach time not worse. The report gives both tables. If a target is
  missed, say so plainly with the numbers and what was tried; do not tune the benchmark to pass.
- No user-visible behavior changes: drawing, selection, copy, word and link lookups, snapshot and the e2e helpers work
  as before, and the existing grid tests pass on both engines.
- Compression of older history stays out of scope, as the entry says; the report may note whether the numbers suggest it
  is worth measuring.
- `perf` PRs with a changelog fragment (one entry for the plan; later PRs edit the earlier fragment rather than adding
  another). The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **Binary for screen and history messages only**; hello, state, clipboard and the page's input stay JSON. The
  alternative was every message in both directions.
- **Hand-written JavaScript decoder**, tested against Rust-encoded fixtures and a round-trip test. The alternative was
  the Rust decoder compiled to WebAssembly for the grid terminal; it was set aside because the grid terminal is plain
  JavaScript loaded apart from the Dioxus bundle, so it would need its own module, build step and loading path, and the
  bounds risk is contained by JavaScript's memory safety plus the fixtures.
- **One plan, one stack**: benchmark, then encoding, then storage. The alternative was two plans.
- **The benchmark is an opt-in tool** reporting WebKit and Chromium memory (settled, and peak during attach), attach
  size and attach time. The alternative was a regular test with thresholds.
- **Success has no fixed number**: memory clearly lower in all three cases, attach size lower, attach time not worse.
  The alternative was matching or beating xterm.js in every case.
- **The related TODO entry about a page-kept back buffer stays separate**: this plan changes how history is encoded and
  stored, not where it comes from.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's terminal and flow-control promises (no output is ever lost; the page may
skip frames but never history); SPEC_impl.md's grid stream sections; `crates/farhelm-proto/src/lib.rs`'s protocol
version rules; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and
FLAKES.md"; `.agents/test-authoring.md`; `.agents/validation.md`.

**Planner proposals:** the tag-byte discrimination, the per-line index, the identity token, keeping the screen decoded,
inline styles, and the PR split below. The planning review's arithmetic puts inline styles (each run carrying its own
style bytes) at roughly 35-60 MB in the worst stream against 737 MiB today, so no style table is needed; a table that
persists across messages would add decoder state that must survive `history_reset` and reconnects, so take one only if
the numbers show inline styles fall short. No decode cache: no requirement needs one, and the identity token covers row
identity.

## Outline

1. `plan/grid-binary-stream/01-benchmark`, typed `test`: the benchmark tool, its streams generated through the Rust
   encoder at run time, and how to run it; baseline numbers in the working log.
2. `plan/grid-binary-stream/02-binary-encoding`, typed `perf`: the binary layout in the proto crate, the supervisor's
   encoder and packing, the JS decoder feeding today's decoded-row model, the fixtures and corpus, the test-only JS
   encoder and the converted JS tests, `grid_tests.rs`, the protocol bump, SPEC_impl.md; the changelog fragment.
3. `plan/grid-binary-stream/03-compact-history`, typed `perf`: compact history storage with on-demand decoding and the
   identity token; edits the fragment; the after numbers; the TODO.md removal.

The helm needs no change beyond what its byte-based pause thresholds call for; keep the 32 KiB message target and the 2
MiB pause unless the benchmark shows a problem, and log a DECISION if you change them.

### Validation

`cargo nextest` for `farhelm-proto`, `farhelm-supervisor` (grid output and stream), `farhelm-helm` (terminal relay) and
the `farhelm` e2e terminal tests that decode the stream, per the pinned setup in `.agents/validation.md`; `node --test`
in `crates/farhelm-ui/js-tests`; the grid-client Playwright project and the browser grid specs
(`e2e/tests/grid-*.spec.ts`, the terminal and link specs) on the engines `.agents/validation.md` names, WebKit included;
all runtime tests through `scripts/record-test-run.py`; `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md`; `cargo clippy` and `cargo fmt --check`; `python3 releasing/check-changelog.py format`.

## Fallbacks

Examples of departures that need the scope reassessment: making page-to-supervisor input binary, a WebAssembly decoder,
compression, a style or link table, a decode cache, changing drawing, changing what the supervisor sends as history,
retuning flow control, or keeping `frame` as JSON. The protocol version is also bumped by other queued plans (such as
`mux-chunk-size`); on a rebase conflict over the number, take the next free one.

Decisions to log: the binary layout and the alternatives considered, how memory is read in each engine, the tag scheme,
the decode cache's size and identity token, any threshold change, every missed target, and every review finding you
decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-grid-binary-stream-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/grid-binary-stream/<nn>-<short-name>`, as
  the outline names them.
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
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Decisions

### 2026-11-09: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-09 (claim 5fd194).
>
> ### Typical Chromium memory did not clearly improve
>
> Opening or reconnecting to a new terminal transfers its screen and scrollback to the page. The old JSON stream and
> fully decoded scrollback were expensive, especially when every cell had a different color. This plan was to replace
> both representations together, preserving drawing, selection, copying and links. The agreed success criterion requires
> clearly lower settled resident-memory growth for typical, indexed-color and true-color output in both Chromium and
> WebKit, smaller transfers and attach time no worse.
>
> The three draft PRs implement the benchmark, binary screen/history messages and compact scrollback with decoding on
> demand. Three interleaved main/candidate pairs at 12,000 history lines and 200 columns give these medians, in MiB of
> resident growth after requesting garbage collection:
>
> | Engine   | Output        |  Main | Candidate |
> | -------- | ------------- | ----: | --------: |
> | Chromium | Typical       |  29.2 |      29.0 |
> | Chromium | Indexed color | 185.3 |     101.7 |
> | Chromium | True color    | 313.8 |     100.3 |
> | WebKit   | Typical       |  63.2 |      59.7 |
> | WebKit   | Indexed color | 727.0 |     265.9 |
> | WebKit   | True color    | 896.2 |     296.9 |
>
> Typical Chromium's exact medians are 30,617,600 versus 30,457,856 bytes, only 0.52% lower. The raw ranges overlap:
> 30,138,368–30,732,288 versus 29,851,648–31,952,896 bytes. That is not evidence of a clear reduction, so the full
> target is unmet. Transfers are 16% smaller for typical output and 41%/53% smaller for the two colored streams;
> attach-time medians improve in all six cases. The committed comparison includes both full metric tables, every raw
> sample and the limits of Linux process-RSS measurements; these are not Mac desktop measurements.
>
> The work already avoids decoding printable text merely to validate it, after profiling identified that allocation
> cost. A further bounded attempt removed a permanent byte-view wrapper from every stored line while preserving each
> line's independent allocation. Neither attempt established a clear typical Chromium memory reduction. The benchmark
> workloads, accounting and thresholds were not changed to obtain a pass. The original TODO entry remains.
>
> May this plan finish with typical Chromium memory treated as essentially unchanged, or must it continue until that
> case clearly improves too? I recommend accepting this narrow exception: the measured colored-history gains and lower
> transfer size/time are substantial, while the original all-six memory criterion remains unsatisfied without that
> explicit decision. A resumed executor would record the exception in its final report and remove the addressed TODO
> entry only after completing the plan under that decision.
>
> The alternative is to keep the original target and authorize further storage investigation. Sharing immutable message
> storage between lines is one possible experiment, but would change the present per-line release contract: evicting one
> line could retain its bytes until the last line sharing that message is gone. Compression, caches and changes to
> drawing remain outside the agreed scope. No such experiment has been implemented or measured, and success is
> uncertain.
>
> The reviewed drafts remain open and must not land as a completed plan until this is answered:
> [benchmark #3245](https://github.com/scode/farhelm/pull/3245/changes),
> [binary transfers #3252](https://github.com/scode/farhelm/pull/3252/changes), and
> [compact scrollback and comparison #3255](https://github.com/scode/farhelm/pull/3255/changes).

The maintainer's answer:

The maintainer decided on 2026-11-09, in conversation with the landing monitor: "just because its not better in the
normal case doesn't invalidate the goal of cutting the extremes".

Finish the plan with typical output's memory treated as essentially unchanged. The goal is cutting the extremes, and the
measured reductions for indexed-color and true-color history on both engines, the smaller transfers and the faster
attach times meet it. Typical Chromium memory staying within baseline variation does not fail the plan. Do not run
further storage experiments (such as sharing message storage between lines) for the typical case. Record this exception
in the final report, and remove the addressed TODO entry when the plan completes.
