# Close or formally accept five very unlikely terminal races

Written against main at d81a3980 on 2026-11-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute five triage outcomes recorded on 2026-11-04, per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute
triage outcomes" (one PR per outcome; a `fix spec+code` outcome is never split). Their `TRIAGE_OUTCOMES.md` entries are
the binding statement of what to do; read each in full from `main@origin`, together with its feedback file under
`review_feedback_queue/`:

1. `registry-test-basename-lookup-can-kill-foreign-host.md` (`fix code`)
2. `colliding-start-cleanup-ends-winning-terminal.md` (`fix code`)
3. `displaced-input-pump-can-submit-replacement-typing.md` (`fix code`)
4. `resize-journal-exhaustion-does-not-trigger-checkpoint.md` (`fix spec+code`)
5. `async-process-verification-leaves-stale-signal-authority.md` (`other`: a FILTER.md accepted case)

All five are very unlikely in practice; the ledger entries give each one's trigger. The maintainer's decision, on the
2026-11-04 highest-bucket triage page: "all of the ones with a very straight forward fix (including that terminal host
what-conts-towards-size things) - just go ahead and fix them. for each in the list that isn't a straightorward fix, add
them to the FILTER.md as a specific case we have accepted."

### The straightforward test, and the fallback

Items 1-4 are recorded as fixes because triage judged them straightforward. Each entry's completion criteria carry the
maintainer's fallback: if the fix turns out not to be straightforward (over about 100 lines of diff, not counting
`TRIAGE_OUTCOMES.md`, the queue removal or a changelog fragment, or it needs a design choice the entry does not settle),
do not fix it. Instead that item's PR adds a `review_feedback_queue/FILTER.md` entry accepting that specific case (and,
for item 4, still corrects the spec sentences the entry names), records why in the ledger's Execution field, and removes
the feedback file. Taking the fallback is a DECISION to log with the evidence (the diff size reached, or the design
choice that came up); it needs no block. A fix that would need any change under `crates/terminal-host/` is not
straightforward and takes the fallback. A fallback PR is `docs:` with no changelog fragment; its bookmark keeps the
outline's name.

### What each PR does

1. **Registry test.** `crates/farhelm/tests/terminal_host_registry.rs` (around `host_process`, 62-67, and the SIGKILL at
   131-136) finds its host by socket basename through `socket_process`
   (`crates/farhelm-supervisor/src/hosts/process.rs`, `may_name_socket`, around 178-186), which can match another
   directory's host. Identify the host by the exact socket path or by the peer process of a connection to the test's own
   socket before signalling. `test:` type.
2. **Colliding start.** `crates/farhelm-supervisor/src/hosts/spawn.rs`: `free_name` (around 612-626) checks only absence
   of a random 40-bit name; when a start does not finish, `finish` (around 403-483) calls `end_unreported_host` (around
   568-605), which takes over and sends Shutdown to whatever answers. Shut down only a host whose launch tag is the one
   this start wrote; leave a foreign or unreadable one alone and report that something may remain. Test with a foreign
   host answering at the socket. `fix:` with a changelog fragment.
3. **Late Enter after a takeover.** `crates/farhelm-supervisor/src/service/handlers.rs` (takeover around 1757-1778, the
   replacement installed around 1971-2017) drops the old attachment without awaiting its input task;
   `crates/farhelm-supervisor/src/service/grid_stream.rs` (`HostAttach` drop around 1175-1182, `Pump::run` around
   1207-1248) only sends a cancel. Await the old input task after cancelling, in takeover, replace and detach, before
   the replacement is installed or detach returns. Check the pump never takes the attachments lock (deadlock). The test
   needs a seam that holds the old pump mid-send. SPEC.md, "One GUI at a time", asks for easy fixes to such races.
   `fix:` with a changelog fragment.
4. **Resizes and history saving.** `crates/farhelm-supervisor/src/hosts/terminal/checkpoint.rs`: `Checkpoints::due`
   (around 187-191) and `exit_tail` (around 83-91) count output bytes only; the host charges 64 bytes per resize record
   (`crates/terminal-host/src/protocol.rs` around 49, `crates/terminal-host/src/journal.rs` around 231-240 and the trim
   at 475-509). Make size changes count toward a checkpoint becoming due and toward an orderly stop's decision to save,
   at the host's per-resize cost, or have the skip check that the host still holds all output since the snapshot; change
   SPEC_impl.md's "The skip counts only processed output bytes, not resizes" (around "Across supervisor restarts") and
   the "a resize alone is not due" unit test to match. `fix:` with a changelog fragment.
5. **Process-number reuse during a thread handoff.** No code change. Add a FILTER.md filter for this specific case: a
   process number reused inside Farhelm's internal handoff between verifying a terminal host's identity and signalling
   it during forced termination (`crates/farhelm-supervisor/src/hosts/process.rs`, `terminate_within` around 445-476).
   Follow the existing filters' shape: a heading naming the case, an "Added 2026-11-04; accepted by the maintainer."
   line, then when it matches and what it does not cover. `docs:` type.

Items 1-4's fallback FILTER.md entries, if any, use the same shape. When several PRs edit FILTER.md, each adds its own
section; keep later PRs rebased cleanly on earlier ones.

### Acceptance criteria

- Five PRs, each meeting its ledger entry's completion criteria (or, for 1-4, the fallback above), each recording its
  execution in the ledger, and each removing its feedback file and index line.
- Code changes go through the existing modules named above; no harness comparisons in shared code (root `AGENTS.md`,
  "Code rules").

## Decisions already made

- **The outcomes**, chosen by the maintainer on 2026-11-04 (quoted above), including the FILTER.md fallback.
- **Grouping:** the maintainer approved these five as one plan on the 2026-11-04 planning page; they share the FILTER.md
  fallback, so one stack keeps those edits ordered.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the five `TRIAGE_OUTCOMES.md` entries named above.

**Binding repository constraints:** no change under `crates/terminal-host/` (its `SPEC.md` "Change policy" makes any
such change costly: a fix that would need one takes the FILTER.md fallback); SPEC.md "Lifecycle operations" (Confirmed
2026-10-10) and "One GUI at a time"; SPEC_impl.md "Across supervisor restarts"; `review_feedback_queue/FILTER.md`'s own
rules; root `AGENTS.md`, "Code rules", "Tests and validation", "Changelog fragments", "Conventional Commits";
`.agents/test-authoring.md`; `review_feedback_queue/.agents/execute-outcomes.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): the ledger order above puts the
trivial test first and the FILTER-only item last; for item 4, prefer counting resize cost in the supervisor (it already
knows each applied resize) over asking the host for its tail position, unless the host already reports it.

## Outline

Five PRs, bottom to top:

1. `plan/unlikely-races-nov4/01-registry-test-exact-socket`
2. `plan/unlikely-races-nov4/02-start-cleanup-checks-tag`
3. `plan/unlikely-races-nov4/03-takeover-joins-input`
4. `plan/unlikely-races-nov4/04-resizes-count-toward-save`
5. `plan/unlikely-races-nov4/05-filter-handoff-pid-reuse`

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy on the touched crates
with `--all-targets`, and through `python3 scripts/record-test-run.py`: the `terminal_host_registry` test (item 1); the
"terminal host spawning, client, discovery and cleanup" selection (item 2); the "grid streams" selection and the
takeover tests in "host terminal switch points" (item 3); the `hosts::terminal::` checkpoint tests and "host terminals
across supervisor restarts" (item 4), each with the pinned tmux when it needs it; `dprint check` on changed Markdown;
`python -B scripts/check-test-sleeps.py` when tests change.

## Fallbacks

Signs an item is not straightforward, which the fallback above answers without a scope reassessment: any change under
`crates/terminal-host/`, a socket-name reservation file or registry, pidfd-based signalling, or a takeover protocol
change visible to clients.

Decisions to log: for each of items 1-4, whether it was fixed or took the FILTER.md fallback and why; the seam used in
item 3's test; the counting approach in item 4; and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-unlikely-races-nov4-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/unlikely-races-nov4/<nn>-<short-name>`,
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

The plan is complete when a linear stack of draft PRs exists, one per triage outcome in the order the outline gives,
each meeting that outcome's completion criteria in `TRIAGE_OUTCOMES.md` (as refined by Decisions already made above),
each having passed its review gate, and each recording its execution in the outcome's Execution field (change id,
bookmark and PR URL) and removing its feedback file and `review_feedback_queue/INDEX.md` line, per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes". Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
