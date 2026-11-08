# Paint a terminal snapshot straight into its blob, halving typical peak memory

Written against main at 5331b984 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Paint a terminal snapshot straight into its final blob.** The snapshot encoder in
> `crates/farhelm-terminal-emulator/src/snapshot/` paints the escape-sequence stream into its own buffer and then copies
> it, with the fixups, into the blob it returns (`container` in `snapshot/v1/mod.rs`), so a snapshot's peak memory is
> about twice the blob, up to 32 MiB at the 16 MiB limit. Painting straight into the blob's buffer after a placeholder
> header, then patching the stream length (the header's last field), would bring that to about one blob plus fixups with
> byte-identical output. Left out of `faster-snapshot-encoder`, which is about speed; `tests/memory.rs` is the arbiter
> of the memory claim.

The maintainer picked this on the planning picker after an assessment, and chose to plan it for the typical case only.

What the planning assessment found, from a read of main at 5331b984 (line numbers approximate; correct them to the truth
as you go):

- **No obstacle in the format.** There is no compression or checksum over the stream; the only hash is the width-table
  fingerprint, which does not depend on the stream (`snapshot/mod.rs` around 235, `v1/mod.rs` around 331-336). Every
  header field except the stream length (offset 30..34, `v1/mod.rs` around 337) is known before painting starts: flags,
  history and trimmed_for come from the arguments (around 218-231, 254-259), size and width table from the terminal.
- **The size limit needs no retry change.** When the limit is crossed the writer drops its buffers and keeps counting
  (`paint.rs` around 880-887), and the size ladder repaints from scratch on every call (`snapshot/mod.rs` around
  444-503). Nothing reads back into or edits the stream once written (`paint.rs` around 858-860, 883, 1149).
- **The caveat.** Cell fixups come after the stream in the blob but are produced while painting (`paint.rs` around
  908-921, into `fixups.cells`), so they still have to be copied once the stream is finished (`v1/mod.rs` around 341).
  Peak memory becomes blob plus cell fixups: about one blob for ordinary terminals, but still near twice the blob for a
  snapshot made mostly of cell fixups. The memory test `a_snapshot_of_fixups_stays_within_its_transient`
  (`tests/memory.rs` around 353-374) forces over 12 MiB of fixups and would stay around 28-32 MiB, so
  `SNAPSHOT_PEAK_MIB = 36` (`tests/memory.rs` around 48) and the "up to 32 MiB" ceiling in the crate docs (`lib.rs`
  around 83) stay.
- **Capacity.** Today the buffers are shrunk and the blob allocated at exactly its length (`v1/mod.rs` around 316-322),
  so no spare capacity stays with the blob. Painting in place needs the reservation cap to allow for the header and
  suffix (`paint.rs` around 893-903, 1124), one `reserve_exact` before appending the suffix, and a shrink at the end. On
  glibc a 16 MiB buffer is mmap-backed, so growing or shrinking it should not copy, but that depends on the allocator;
  reserving the full limit up front avoids growth, since untouched pages are not resident.
- **The format is frozen.** Version 1 is what running hosts keep for as long as they run (`v1/mod.rs` around 1-5), so
  the output must be byte for byte identical. The terminal host (`crates/terminal-host/`) stores blobs without reading
  them and does not depend on the emulator crate.

### Acceptance criteria

- **Byte-identical output** for every snapshot: the golden blobs in `tests/snapshot_golden.rs` (fixtures under
  `tests/fixtures/snapshots/v1/`), `snapshot_roundtrip.rs` and `snapshot_fuzz.rs` pass unchanged.
- **Typical peak memory about halved:** the stream is painted into the blob's own buffer after the header, the stream
  length is patched afterwards, and the suffix (fixup count, cell fixups, cursors, REP) is appended to that buffer
  rather than everything being copied into a new one. `typical_output_at_full_scrollback` in `tests/memory.rs` gets a
  tighter assertion for the low-fixup case that locks in the gain. Its blob is only about 3 MiB (the crate docs record
  about 7 MiB peak for ordinary output, `lib.rs` around 92), so the bound is relative to the measured blob (for example
  `peak <= blob + blob / 2`, tuned from a run), not an absolute figure, with the same headroom premise the restore test
  has (around `tests/memory.rs` 414-421: the gap between the earlier high-water mark and current resident memory is at
  most 1 MiB, or the measurement hides growth). Run the new assertion once against the unchanged encoder and record in
  the report that it fails there, and what both runs measured.
- **The reservation cap is load-bearing:** once the buffer also holds the header or must take the suffix, the needed
  size can exceed the payload budget, and `target - buffer.len()` in `Writer::reserve` (`paint.rs` around 897-903) would
  underflow. Raise the cap and pin it with a budget test at the exact limit, in the shape of
  `crossing_at_raw_print_or_cell_write_frees_both_buffers_and_keeps_counting` (`paint.rs` around 1483).
- **The worst case is unchanged and documented:** the fixup-heavy test keeps its limit, and the crate docs (including
  the "about 7 MiB" sentence around `lib.rs` 92) say what ordinary output now takes and that a snapshot made mostly of
  cell fixups still peaks near twice the blob.
- **No spare capacity kept with the returned blob**, as today.
- **TODO.md:** the last PR removes the Near term entry quoted above and adds a Maybe later entry recording the remaining
  worst case: that a snapshot made mostly of cell fixups still peaks near twice its size, because cell fixups are built
  in their own buffer and copied after the stream, and that painting them in place too would need a separate design. The
  maintainer asked for this entry on the planning page.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments", since a `perf` title
  requires one. Expect `kind: none` with the reason (a few MiB less for a moment during a checkpoint is not something a
  user notices); use `kind: changed` only if the measurements show otherwise.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **Plan the typical case only.** The worst case keeps today's peak and the documented ceiling stays.
- **Record the remaining worst case as a Maybe later TODO entry** (the alternative was leaving it to the code docs and
  the report).
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** the frozen snapshot v1 format (`snapshot/v1/mod.rs` header comment) and
`crates/terminal-host/SPEC.md`'s change policy (blobs outlive releases); the crate docs' memory statements; root
`AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md";
`.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- `Painter::new` starts the writer with a buffer that already holds the header with a placeholder stream length;
  `Writer::reserve` and the payload budget allow for the header; `container` patches the length and appends the suffix
  instead of building a new buffer; `encode` and `encode_history` work out the header before painting. The `stream_len`
  counter (`paint.rs` around 849) stays the source of truth.
- Expect about 40-80 changed lines in `snapshot/v1/`, plus the tests that build a `Painted` by hand (`v1/mod.rs` around
  562) or check `out` (`paint.rs` around 1467-1522).
- A smaller alternative, also acceptable: leave `Painter::new`, `encode` and `encode_history` alone, raise only the
  reservation cap so the paint buffer has room for header and suffix, and in `container` shift the stream up by the
  header size in place (`copy_within`) and write the header in front. That costs one memmove of the stream, the same
  byte work as today's copy, with no second buffer, and keeps the deferred-allocation test (`paint.rs` around 1112-1113,
  1464-1468) and the hand-built `Painted` test as they are. Pick whichever is clearer and log the DECISION.

## Outline

One PR, `plan/snapshot-in-place-paint/01-paint-into-blob`, typed `perf`: the encoder change, the tightened memory
assertion, the crate docs, the changelog fragment and the TODO.md changes.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy, the
emulator crate's tests including the golden, roundtrip and fuzz tests, and the ignored memory tests in `tests/memory.rs`
run in release mode, one process per test, with the command in that file's header, through
`python3 scripts/record-test-run.py`; `python -B scripts/check-test-sleeps.py` with the interpreter
`docs/test-sleep-check.md` prepares if you touch tests.

## Fallbacks

Examples of departures that need the scope reassessment: painting cell fixups in place too, changing the snapshot format
or its version, changing the size ladder or the 16 MiB limit, or adding a custom allocator.

Decisions to log: how the buffer is reserved and shrunk, the new memory assertion and the measurement behind it, the
changelog kind, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-snapshot-in-place-paint-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/snapshot-in-place-paint/<nn>-<short-name>`, as the outline names them.
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
