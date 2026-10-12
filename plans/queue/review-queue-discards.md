# Execute twenty-one triage outcomes: remove the discarded review-queue findings

Written against main at d3b28820 on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Carry out twenty-one triage outcomes recorded in root `TRIAGE_OUTCOMES.md`, each outcome `discard`, in one draft PR.
Their headings, which are also the feedback file names under `review_feedback_queue/`:

- `wedged-browser-teardown-test-pass-browser-cleanup-finishes.md`
- `wedged-browser-test-accepts-detach-handler-completion.md`
- `wedged-browser-test-accepts-independent-cleanup-signal.md`
- `wedged-browser-test-may-encounter-queue-backpressure-blocked.md`
- `concurrent-first-use-barrier-guarantees-disputed-interleaving.md`
- `replacement-tests-overstate-immediate-list-no-delete-evidence.md`
- `shutdown-race-test-releases-sink-shutdown-starts.md`
- `host-identity-race-test-pass-exercising-competing-writes.md`
- `stop-recovery-test-fail-wrong-listing-request.md`
- `client-task-scheduling-mistaken-server-side-overlap.md`
- `retained-failed-tags-become-mandatory-upgrade-test-inputs.md`
- `dropping-upload-guard-outside-tokio-runtime-skips.md`
- `oversized-successful-agent-reply-becomes-internal.md`
- `unbounded-aggregate-capture-replies.md`
- `hostnotfound-refresh-keeps-serving.md`
- `legacy-launch-validation-could-prevent-startup.md`
- `service-start-timing-could-evade-restart-detection.md`
- `watchdog-logs-rather-recovering-window.md`
- `browser-children-retain-detached-descendants-every-platform.md`
- `descendant-surviving-markeddecoy-destruction.md`
- `omp-context-changes-initial-eligibility-check.md`

A `discard` outcome means only "not worth the maintainer's time at this time"; it says nothing about whether the finding
is right (root `AGENTS.md`, Triage step; `review_feedback_queue/AGENTS.md`, Lifecycle). Executing one removes the
feedback file and its index line and records the execution, with no code or spec change. Nothing changes for someone
running Farhelm.

The maintainer decided these as classes rather than one by one: test-strength findings whose fix would need a new
test-only hook in product code, or that are purely speculative; product findings with no trigger a shipped configuration
can reach; release tooling that fails safe; and non-Linux test-harness gaps that are already documented. Each ledger
entry's Assessment and Decision say which.

Acceptance criteria:

- One draft PR removes all twenty-one feedback files and their lines in `review_feedback_queue/INDEX.md`, and sets each
  outcome's `TRIAGE_OUTCOMES.md` Execution field to `complete` with the jj change ID, bookmark and PR URL.
- The PR changes nothing else: no code, no spec, no other ledger field.
- `review_feedback_queue/INDEX.md` still matches the files in the directory afterwards, and `dprint check` passes on the
  changed Markdown.
- The PR passed the review gate, is not marked ready, and nothing is merged.

## Requirement sources

**The user's triage decisions (2026-10-11):** recorded in root `TRIAGE_OUTCOMES.md`, which landed in #1919, one entry
per feedback file. For the test-strength findings the user chose to fix the weak tests but "skip hard ones": discard
those whose fix needs a new test-only hook in product code, and the purely speculative ones. For product findings with
no reachable trigger the user said "fix SUPER cheap ones with complexity gate", and that anything breaking a security
boundary "must not be silently discarded"; the entries discarded here were judged neither cheap nor security-relevant.
For tooling the user said "fix everything small that's a legit bug, discard rest". For the non-Linux harness gaps the
user chose "Gate e2e to Linux" and accepted the documented browser-harness limitation.

**The user's plan-time decisions (2026-10-11):** "use the planning system to start planning all of the already triaged
entries"; the user chose "8 plans as proposed", of which this is the discards plan; "One PR per plan", which overrides
root `AGENTS.md`'s one-PR-per-outcome rule for these outcomes only; review gate "gpt-6.1-sol high"; no-workhorse mode,
which `plans/AGENTS.md` requires.

**Planner proposals:** re-reading each outcome before removing it, and the narrow validation below.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work, documentation-only changes;
Execute triage outcomes, as overridden above; Agent scratch space), `plans/AGENTS.md` (Executing one plan), and
`review_feedback_queue/AGENTS.md` (Lifecycle).

## Outline

- Before removing anything, re-read each outcome's ledger entry on the main you are building on and confirm its Outcome
  is still `discard` and its Execution still reads ``planned in `plans/queue/review-queue-discards.md` `` (the planning
  PR that queued this plan set it so, per `plans/AGENTS.md`, Planning triage outcomes). If an entry has changed (a
  different Outcome, a different plan named, `complete`, or any other Execution text), leave that item alone, log a
  DECISION, and say so in the report; do not re-triage it.
- Delete each feedback file under `review_feedback_queue/` and its line (with any continuation line) in
  `review_feedback_queue/INDEX.md`. If a file is already gone from main, record that and still complete its ledger
  entry.
- Set each Execution field to `complete`, with the jj change ID and bookmark, then add the PR URL in the same change
  once the PR exists and push again; no separate bookkeeping PR.
- The PR type is `chore:`. It has no user-visible effect, so per root `AGENTS.md` (Releases and the changelog) it adds
  no changelog fragment. The PR description may stay empty.

### Validation

This is a documentation-only change in the sense of root `AGENTS.md` (Finishing work): run no Rust, JavaScript, browser,
desktop, installer or provisioning tests. Run `dprint check` on `TRIAGE_OUTCOMES.md` and
`review_feedback_queue/INDEX.md`, and confirm by listing the directory that every remaining feedback file has exactly
one index line and every index line names an existing file.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-review-queue-discards-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution fields on the PR) rather than starting over. If it does not exist, this is a
fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new review launch. A dead monitor or a heartbeat stale for two
sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an instruction
to act: stop launching work, remove scratch you own, wait for or stop the job most likely responsible, and resume only
when the watchdog reports headroom. Record the watchdog's handle, watched paths, status path and delivery mechanism in
the log, include its state in every handoff note so a resumed session reconciles or restarts it, and stop it when the
plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/review-queue-discards/01-discards`.
- All of this plan's outcomes go in one commit, one bookmark and one draft PR, per the user's grouping decision. Within
  this run, if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits; this PR is `chore:` with no changelog fragment.
- Record the change ID and bookmark in the ledger before creating the PR, then add the URL to the same change and push
  again.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  the PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: this file's goal and outline, and the twenty-one ledger entries. Ask
the reviewer to check specifically that every removed file was a `discard` outcome whose Execution on main read
``planned in `plans/queue/review-queue-discards.md` ``, that no other feedback file or index line was touched, and that
the index still matches the directory. Where you disagree with a finding, decide on the merits and log the DECISION. Do
not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

This plan has no implementation to depart from. Anything beyond removing these twenty-one queue items and recording
their execution (a code fix, a spec edit, touching another queue item) is out of scope. If you believe one is needed,
run a fresh-context review through galaxy-brain with this charter, supplying the ledger entries, the user's decisions
above, this outline and the proposed departure:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

A departure is not authorized by that review; record it in the report as a possible follow-up instead.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular any outcome found changed or already removed on main, and every review finding
you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine forks within this scope and log them. If current main has changed so that one of these decisions no
longer applies (the entry was revised or already executed), leave that item out, log it, and name it in the report's
open questions; never re-triage an item yourself. Block per `plans/AGENTS.md` (Executing one plan, step 10) only when no
outcome is left to ship or the PR itself cannot proceed without the user.

You never ask the maintainer anything in conversation while running this plan: not a direct question, not an optional
one, not an offer of more work, and not a turn that ends waiting for a confirmation. Every fork is either decided by you
and logged as a DECISION, or, when only the maintainer can decide it, a block per `plans/AGENTS.md` (Executing one plan,
step 10). Things the maintainer should know and possible follow-ups go into the plan's report, written as statements.

## Done criterion

The plan is complete when its one draft PR exists, carries out every outcome listed above that is still a `discard`
planned in this plan on main, meets the acceptance criteria above, and passed the review gate. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
