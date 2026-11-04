# Make the checkpoint burst at an orderly supervisor stop gentler

Written against main at 856099b8 on 2026-11-03. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except for the one SPEC_impl.md sentence the maintainer decided to reverse (Decisions already made).

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **An orderly supervisor stop encodes a checkpoint of every busy terminal at once.** At an orderly stop (an update, the
> desktop app replacing its supervisor, a SIGTERM, SIGINT or SIGHUP), `Supervisor::stop_in_order` runs
> `checkpoint_hosts_for_exit` (`crates/farhelm-supervisor/src/service/core/host_handover.rs`, module docs "Stopping")
> beside the closing of tmux's output clients, both within 10 s (`HOST_CHECKPOINT_BUDGET`, equal to
> `SHUTDOWN_OUTPUT_BUDGET`, in `crates/farhelm-supervisor/src/service/core.rs`). Every terminal host runtime with any
> output since its last snapshot is asked for a checkpoint, however little that output is, and each checkpoint encodes
> the whole terminal, not the new output: up to about 2 s of a core at full scrollback in true color, over 10 s for one
> colored like an image (the entry about typing pausing during checkpoints). With many sessions that is a burst of
> whole-terminal encodes, often while the machine is already busy with an update. #2897 bounded it: at most as many
> encodes at once as `std::thread::available_parallelism` reports (a semaphore of that size), all under the one
> deadline, and closing a runtime cancels its encode between rows (`HostTerminal::abort`,
> `farhelm_terminal_emulator::Cancel`), so the stop and the process's exit stay within the budget. What is left: for up
> to 10 s every core can be busy encoding; and a terminal whose checkpoint did not finish or never got a core keeps its
> previous snapshot and all the output since, which the next supervisor replays (a slower recovery, the same terminal),
> where which terminals get a core first follows the session map's order, not how much replay a checkpoint would save.
> The largest lever needs the maintainer: skipping the stop checkpoint of a terminal whose output since its snapshot is
> small (it replays in milliseconds) removes most of the burst, but the terminal-host work required a snapshot on every
> orderly stop, and SPEC_impl.md ("Across supervisor restarts") states that every host with output since its snapshot is
> checkpointed however little that output is. Changes that need no such decision: a faster encoder (the entry above),
> checkpointing the terminals with the most output since their snapshot first, and fewer slots than cores so a stop does
> not take the whole machine. Keep the stop bounded by its deadline, keep cancelling encodes as runtimes close (an
> encode runs on tokio's blocking pool, which the exiting process waits for while it still holds the state directory's
> claim), and keep the `stopping` fence that stops re-adoption, creates and Restarts from publishing a connected runtime
> after the gather. No test measures the burst; to see it, stop a supervisor holding many full terminals (12,000 lines
> of true-color history each) and watch CPU and the "left a terminal host its previous snapshot at exit" log lines.

NOTE: "The entry above" (the faster encoder) is not part of this plan. It is planned separately as
`faster-snapshot-encoder`, which may run at the same time as this one (Coordination below).

### What is known

From a read of main at 06c1e7cd during planning (main has since moved only by a TODO.md edit; line numbers are
approximate):

- **Where the stop runs:** `stop_in_order` (`service/core.rs` around 18704-18727) joins `checkpoint_hosts_for_exit`
  (`service/core/host_handover.rs` around 254-361) with closing tmux's output clients. `HOST_CHECKPOINT_BUDGET` and
  `SHUTDOWN_OUTPUT_BUDGET` are around 18526-18537 of `service/core.rs`. When the result is false, `stop_in_order` logs
  at `info` (around 18715-18720).
- **What the stop does today:** it gathers every host runtime under the session map's lock and sets `stopping` (around
  260-269), keeps the `still_driven` ones (around 286-290, `still_driven` around 1101), and spawns one task per host
  into a `JoinSet`. Each task awaits a permit from a `Semaphore` sized by `available_parallelism` (around 299-300) until
  the deadline, then calls `HostTerminal::checkpoint_now(deadline)`, then `abort()`. Outcomes are logged; a `Kept`
  outcome is logged at `info` ("left a terminal host its previous snapshot at exit", around 338-346) and makes the
  return value false. `close_all` (around 482-495) aborts everything and waits until the deadline plus
  `ENCODE_END_GRACE` (1 s, around 234). A runtime whose accepted checkpoint equals its position answers `Current` at
  once (`hosts/terminal.rs` around 3585), so any output or resize since the snapshot means a full encode.
- **The entry's "session map's order" is imprecise.** `Supervisor::sessions` is a `HashMap` (`service/core.rs` around
  4898) and `gather_host_terminals` (`host_handover.rs` around 455-478) iterates its values, and each task then races
  for a permit, so the order is whatever the multi-threaded runtime first polls. It is effectively arbitrary.
- **The slots do not cap all encoding.** `checkpoint_now` waits for an automatic checkpoint already encoding in that
  runtime (`hosts/terminal.rs` around 2396-2404), and runtimes still waiting for a slot keep running their own automatic
  encodes. So today's burst is the slots plus every automatic encode already in flight. This plan does not change that
  (see "Not in this plan" below). The runtime's count of running encodes is the `running` watch in `struct Encodes`
  (`hosts/terminal.rs` around 1262-1264), which only `encodes_ended` (around 2383) reads today.
- **An automatic encode in flight makes a stale pre-check.** A host with an automatic encode in flight when the stop
  gathers it shows the older snapshot in `status()`, so its tail is at least 1 MiB and it is queued. By the time its
  runtime takes up the stop's request, the runtime has settled that encode, had the snapshot accepted, and processed the
  rest of the read the encode interrupted, so its position is a few kilobytes past the new snapshot. The "already
  current" check in the runtime's `checkpoint_now` (`hosts/terminal.rs` around 3585) compares exact positions, so today
  it encodes the whole terminal again for that tiny tail. The same happens to a host whose automatic encode finishes
  between the gather and its slot.
- **The output since the snapshot is readable from the handle, with no round trip:** `HostTerminal::status()` (around
  2038) returns `HostStatus` (around 617-665) with `position: Position` (what the emulator has processed) and
  `checkpoint: Option<Position>` (the latest snapshot the host is known to hold; `None` for none or only an initial one
  at (0, 0)). The tail a skipped checkpoint leaves for the next supervisor, in bytes, is
  `position.offset - checkpoint.map_or(0, |c| c.offset)`. A tail of resizes alone is 0 bytes and replays trivially; a
  Restart-seeded host's seed sits at (0, 0), so its tail counts from 0, which is right; output journaled but not yet
  read is in neither the tail nor any checkpoint; and a checkpoint accepted unseen after its deadline does not matter at
  a stop.
- **The threshold's natural home:** `CheckpointPolicy` (`hosts/terminal/checkpoint.rs` around 52-76), whose floor is
  `CHECKPOINT_MIN_BYTES` (1 MiB, around 36-39; its doc already argues that a 1 MiB replay costs about what an encode
  would). The policy reaches every runtime through `HostTerminalOptions::checkpoints` (`hosts/terminal.rs` around
  939-944), and tests already override it, the e2e suite included through the `FaultHooks::host_terminal_options` seam
  (`service/core.rs` around 7708-7717, used in `crates/farhelm/tests/e2e/host_restarts.rs` around 964). `NotTaken` and
  `CheckpointOutcome` are in the same file (around 88-110).
- **Why the every-stop rule exists:** the repository records only replay length. #2897's commit message ("snapshots
  every host so the next supervisor replays little") and the docstring at `host_restarts.rs` around 280-281 ("without
  the stop's checkpoint, the next supervisor would replay the whole journal since the last one"). Nothing ties the rule
  to the delivery ambiguity or to gap risk. With a 1 MiB threshold and the default 32 MiB journal cap (the only cap
  production uses), a skipped host's tail of output the runtime had read takes at most 1 MiB of the journal, so it
  starts its absence with at least 31 MiB free less whatever it had journaled but the runtime had not yet read (which no
  checkpoint could have covered either). Replay below `delivered_end` stays side-effect free.
- **Tests whose premise is that a stop encodes after any output,** which the skip breaks:
  - `an_orderly_stop_ends_its_encodes_before_its_runtime_drops`
    (`crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs` around 1780-1838): writes `"some output\r\n"`
    and requires the encode to reach its gate;
  - e2e `an_orderly_restart_keeps_a_host_agent_running_with_its_screen` (`host_restarts.rs` around 299-333): expects
    "the stop left the host a snapshot";
  - e2e `a_stop_that_ran_out_of_time_leaves_the_previous_snapshot_and_tail` (around 758-826): requires the held encode
    to be cancelled.
- **Test tools that exist:** the `EncodeGate` (`hosts/terminal.rs` around 945-948) and the stand-in hosts in
  `host_terminals.rs`. No test measures the burst. Two struct literals set `CheckpointPolicy` today (`host_restarts.rs`
  around 964, `host_terminals.rs` around 2032) and need the new field. The recovery-from-tail path already has e2e
  coverage: `a_stop_that_ran_out_of_time_leaves_the_previous_snapshot_and_tail` (`host_restarts.rs` around 758-826) and
  `a_crash_recovers_a_host_terminal_from_its_snapshot_and_tail` (around 388). A host whose stop checkpoint was skipped
  ends in the same state as one whose stop checkpoint timed out: its previous snapshot, the tail after it, and its
  connection closed by the stop.

### Acceptance criteria

- **Skip small tails.** At an orderly stop, a host terminal whose tail (processed output bytes since its latest known
  snapshot, as above) is under the stop threshold gets no checkpoint: no encode, and it keeps its previous snapshot and
  tail for the next supervisor. The threshold is a field of `CheckpointPolicy`, separate from `min_bytes`, defaulting to
  `CHECKPOINT_MIN_BYTES`, so tests can set it (0 restores today's behavior). The rule is applied in two places, both
  reading that field:
  - the supervisor, before a slot: a host whose tail is already under the threshold takes no slot and stays out of the
    ordered queue, and its connection is closed at once (`abort()` when the skip is decided), not left open until the
    final `close_all`, where it would keep reading output the stopping supervisor counts as delivered and could start an
    automatic encode;
  - the runtime's own handling of the stop's request, next to the "already current" check: a host whose tail fell under
    the threshold after the gather (an automatic encode finished first) answers with the skip instead of encoding the
    whole terminal again.
- **A skip is its own outcome, and a success.** It is a new `CheckpointOutcome` variant (for example
  `CheckpointOutcome::Skipped { tail }`), not a reason under `Kept`, because `Kept` means "a checkpoint the stop wanted
  did not happen" in `checkpoint_hosts_for_exit` and its `info` log. A skip counts as success for the return value and
  is logged at `debug`.
- **Largest tails first.** The hosts that are checkpointed get their slots in order of tail bytes, largest first, and
  the order is real: it does not depend on which task the runtime polls first.
- **Fewer slots.** At most `max(1, available_parallelism / 2)` stop encodes run at once.
- **Unchanged guarantees** (the entry's constraints): the stop stays bounded by its deadline; closing a runtime still
  cancels its encode; `close_all` and its grace are kept; the `stopping` fence still stops re-adoption, creates and
  Restarts from publishing a connected runtime after the gather; a host whose checkpoint did not finish or never got a
  slot keeps its previous snapshot and tail, as today.
- **Docs follow the code:** SPEC_impl.md, "Across supervisor restarts" (around 2318-2336) states the skip, the slot
  count and the order, and its sentence "Every host with output since its snapshot is checkpointed however little that
  output is" is replaced. The skip joins the list of reasons a host keeps its snapshot (around 2332-2334). The
  `host_handover.rs` module docs ("Stopping", around 10-55) and the function docs (around 237-253), the
  `HOST_CHECKPOINT_BUDGET` doc (`service/core.rs` around 18528-18537), the new policy field and outcome, and the meaning
  of `stop_in_order`'s `info` log all match.
- **Tests:** the three tests above set the threshold to 0, keeping what they test. New tests cover: a stop skips a host
  whose tail is under the threshold (no encode reaches a gate, the host keeps its old snapshot, the result counts as
  success) and checkpoints one at or over it; the runtime answers the skip when the stop's request finds its tail under
  the threshold; and with one slot and gated encodes, the stop asks the host with the largest tail first. A new e2e test
  of recovery after a skip is optional: the skipped state is the timed-out state the existing e2e tests above already
  recover, and a new tmux test adds runtime to every run of the selection for little new evidence. Add one only if the
  implementation leaves a skipped host in a state those tests do not cover, and log the DECISION either way.
- **Changelog and TODO:** a changelog fragment per root `AGENTS.md`, "Changelog fragments" (this is a user-visible
  `perf` change, kind `changed`). The last code PR removes this TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-11-03, on the planning page:

- **Skip below 1 MiB.** A terminal's stop checkpoint is skipped when its output since its last snapshot is under 1 MiB,
  the existing minimum that triggers an ordinary checkpoint (`CHECKPOINT_MIN_BYTES`). This deliberately reverses
  SPEC_impl.md's sentence that every host with output since its snapshot is checkpointed however little that output is;
  this plan updates that text and the tests that assume it. The reason: the only recorded reason for the rule is replay
  length, and a tail under 1 MiB replays cheaply on top of a restore that happens either way.
- **Slots:** at most `max(1, cores / 2)` stop encodes at once, instead of one per core, so a stop leaves half the
  machine to the update or the desktop app.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort for every PR (see Review gate). If it cannot
  be reached when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute
  reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:**

- SPEC_impl.md, "Across supervisor restarts" (around 2318-2336): what the stop guarantees, apart from the sentence the
  maintainer reversed. SPEC_impl.md, "Recovery and checkpoints" (around 1943-1957), owns the encode timing figures.
  SPEC.md does not describe stop checkpoints and needs no change.
- Root `AGENTS.md`, "The live install is off-limits": never touch the maintainer's own Farhelm; tests make their own
  supervisors. Also "Sharing the machine with other agents", "Tests and validation", "Conventional Commits", "Changelog
  fragments", "TODO.md and FLAKES.md".
- `.agents/test-authoring.md`, `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The supervisor's pre-slot check reads the tail from a handle method beside `status()` (for example
  `HostTerminal::exit_checkpoint_tail() -> Option<u64>`: the tail when at or over the policy's threshold, `None` when
  the stop may skip it), or computes it from `status()` in `host_handover.rs` with only the threshold exposed; either is
  fine. Why both call sites: the runtime's check alone would have every skipped host take a slot, possibly wait out an
  automatic encode, and hold the queue up for hosts that need an encode; the supervisor's check alone is stale for a
  host whose automatic encode settles after the gather (What is known). One rule, one policy field, two call sites.
- A separate policy field rather than reusing `min_bytes`: the three tests need the stop to encode after a few bytes,
  and lowering `min_bytes` for that would make automatic checkpoints fire on nearly every record, so their gates would
  be hit by automatic encodes rather than the stop's.
- `CheckpointPolicy::for_journal_cap` sets the stop threshold to `CHECKPOINT_MIN_BYTES.min(max_bytes)`, so a runtime
  whose host has a smaller journal cap (only tests use one today) does not skip a tail that is most of its journal. One
  expression; no production effect today. Drop it if it complicates a test, and log the DECISION.
- Order by tail bytes, largest first: sort the hosts to checkpoint, then acquire permits in that order in one loop and
  spawn each checkpoint with its permit moved in, rather than letting tasks race for one. Tail alone is the key; tail
  per snapshot byte would need the latest snapshot size published in `HostStatus`, which is not worth it here.
- No prediction: encodes that might not finish in the budget (image-like terminals) are not predicted or skipped. The
  faster encoder addresses those.
- The slot count is passed in (a parameter or a test seam) so a test can run the stop with one slot.

### Not in this plan

Automatic checkpoints already encoding when the stop begins are not counted against the stop's slots; the slots cap only
the stop's own encodes, as the acceptance criterion and the maintainer's decision say. A planning review found that
counting them could not deliver its aim: runtimes waiting for a slot keep starting automatic encodes after the gather,
and bounding those means suppressing automatic checkpoints once `stopping` is set, which changes the automatic
checkpoint policy (a Fallbacks departure). It would also be real machinery (a new accessor for the running-encode count,
permits taken at the gather and released as each encode ends) for little gain: a host with an automatic encode in flight
already has a tail of at least 1 MiB, so it is never skipped and waits out that encode inside its own slot. List this in
the report's "open questions and possible follow-ups", with that reasoning, rather than building it.

### Coordination with `faster-snapshot-encoder`

That plan makes the snapshot encoder faster and touches only the terminal emulator crate. The two plans may run
concurrently and neither depends on the other. Do not restate encode timing figures (seconds per encode, blob sizes) in
any code comment or doc this plan writes; refer to SPEC_impl.md, "Recovery and checkpoints" instead, because the encoder
plan owns those figures and will change them. Where text this plan rewrites quotes them today (`host_handover.rs` around
37-39, `service/core.rs` around 18535-18536, and the "(up to about two seconds ...)" aside in "Across supervisor
restarts"), replace the figure with that reference. On a rebase conflict in SPEC_impl.md with that plan's work, keep
both sides' meaning. Leave the encoder's TODO.md entry alone, even where it mentions this one.

## Outline

1. `plan/stop-checkpoint-burst/01-order-and-slots`: the tail-ordered permits, `max(1, cores / 2)` slots, the injectable
   slot count, the ordering test, the docs for order and slots (`host_handover.rs`, SPEC_impl.md around 2319-2321), and
   the changelog fragment. Type `perf`.
2. `plan/stop-checkpoint-burst/02-skip-small-tails`: the policy field, the supervisor's pre-slot check with its
   immediate close, the runtime's check beside "already current", the `Skipped` outcome and its logging, the three tests
   set to threshold 0, the new skip tests, the SPEC_impl.md reversal (around 2332-2336) and the remaining docs, an edit
   to PR 1's fragment covering the skip, and the TODO.md removal. Type `perf`.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Run runtime tests through
`python3 scripts/record-test-run.py`, with the pinned nextest and tmux setup that `.agents/test-evidence.md` gives.
Expect, per PR:

- the "host terminals across supervisor restarts" selection in `.agents/validation.md` (tmux required; it covers
  `host_terminals.rs` tests by name and every `host_restarts::` e2e test). New supervisor unit tests not matched by its
  filter are run by name too; adding them to that selection is your call (an instruction-file change, so it also runs
  `python3 scripts/check-agents-md.py`);
- the "host terminal runtime" selection, for the `checkpoint.rs` and `terminal.rs` changes;
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings`, and
  `cargo clippy -p farhelm --bins -- -D warnings` (a test seam can hide behind the `test-seams` feature);
- `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares;
- `python3 releasing/check-changelog.py format` and `dprint check` on the changed Markdown.

The deterministic tests (order, slot count, skip) are the evidence. A by-hand measurement of the burst is not required;
if you make one, it runs only against a disposable supervisor started from a temporary state directory, never the live
install.

## Fallbacks

Examples of departures that need the scope reassessment:

- publishing new fields in `HostStatus` beyond what the skip and order need (snapshot sizes, encode durations);
- changing the automatic checkpoint policy, the stop's budget, `close_all`'s grace, or the `stopping` fence;
- a threshold other than the policy's 1 MiB default (the journal-cap clamp under Planner proposals aside), or one that
  depends on the snapshot's size;
- counting automatic encodes against the stop's slots, or suppressing them during a stop ("Not in this plan");
- skipping hosts for any reason other than a small tail.

Decisions to log: how the two skip checks share the rule, the outcome's shape, whether the journal-cap clamp and an e2e
recovery test were added, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-stop-checkpoint-burst-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/stop-checkpoint-burst/<nn>-<short-name>`,
  as the outline names them.
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
alternatives considered: in particular those named under Fallbacks above, any planner proposal you change, and every
review finding you decided not to follow. The user will ask for these later.

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
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
