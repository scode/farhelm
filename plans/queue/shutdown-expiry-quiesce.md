# Execute the shutdown-expiry triage outcome: a bounded last quiet-down on stop, and a spec limit on tmux-crash defenses

Written against main at bd8d5d76 on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this outcome's own PR amends them as described below.

This plan has no dependency on another plan.

## The goal

Carry out the triage outcome recorded in root `TRIAGE_OUTCOMES.md` under the heading `shutdown-expiry.md` (outcome
`fix spec+code`), exactly as root `AGENTS.md` section "Execute triage outcomes" prescribes: one reviewable commit, one
stable bookmark and one draft PR carrying both the spec and the code change.

The feature is a planned supervisor stop (an update or restart, `systemctl stop`, Ctrl-C, quitting the desktop app).
Before exiting, the supervisor tells tmux to stop sending output on each of its terminal-output connections and waits
for tmux to confirm, because closing such a connection with output queued has crashed the whole private tmux server in
the past (observed on distro tmux 3.6 and tmux 3.7b), ending every session on the host. That shutdown runs under one
10-second budget that starts by waiting for a lock ordinary terminal operations hold; when the budget runs out the
supervisor exits anyway, closing whatever is still unquieted abruptly. Whether the pinned tmux 3.7c still crashes this
way is unverified.

The ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do. Read
it and the feedback file `review_feedback_queue/shutdown-expiry.md` before starting. This file adds the plan-time
decisions and the mechanism chosen; where the two seem to disagree, the ledger entry wins and the disagreement is a
DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- One draft PR exists. On budget expiry, the supervisor makes one bounded, best-effort attempt to switch output off on
  its remaining output clients before it exits, covered by one focused test.
- SPEC_impl.md records the principle below in the same PR.
- The code change, including its test, stays within roughly 150 lines. If it cannot, the PR carries only the spec part
  (see Hard complexity bound).
- The PR updates its own `TRIAGE_OUTCOMES.md` Execution field and removes (or, under the bound's fallback, narrows) the
  feedback file and its index line, and passed the review gate if it changes code or tests.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's decision (2026-10-09 triage), in their words:** "fix code with complexity bound, this cannot turn into
something large. at the same time, also fix spec - Until and unless we have evidence of this bug in 3.7c or later we do
not spend significant complexity trying to defend against it. this is the Nth time I'm triaging code review feedback
related to this stupid tmux bug."

**The user's plan-time decisions (2026-10-09):** review gate "gpt-6.1-sol high reviewer, no swarm"; one plan per
outcome; no-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanism, the time limit and the test shape below. A fresh-context planning review
checked them against the code.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Sharing the machine with other agents; Agent scratch space; The live install is off-limits),
`plans/AGENTS.md` (Executing one plan), `review_feedback_queue/AGENTS.md` (Lifecycle), and `.agents/test-authoring.md`
for test changes.

## Outline

Line numbers drift; find the code by name. Verified on main at bd8d5d76.

**Where it happens.** `crates/farhelm-supervisor/src/service/teardown.rs`, `shutdown_output_clients`, wraps the whole
teardown (its first await is the `attachments` lock; `stopping` is set only after the lock is held) in
`tokio::time::timeout(budget, ...)`. Its caller in `crates/farhelm-supervisor/src/service/core.rs` (search
`SHUTDOWN_OUTPUT_BUDGET`) logs "some terminal-output clients did not finish closing in time; exiting anyway" and returns
when it reports false. Process exit then closes every output client abruptly.

**Mechanism: a last quiet-down on expiry.** When the budget expires, before returning, list the private tmux server's
control clients once and switch each to no output, using what the tmux driver already has in
`crates/farhelm-supervisor/src/tmux.rs`: `reap_stale_control_clients` shows the pattern (`list_client_pids_and_flags`,
filter `control-mode`, `disable_control_client_output`), minus its kill. Everything on the private server's control
connections belongs to this supervisor (that docstring says so: "private-server control clients belong to the
supervisor, full stop"), so no roster bookkeeping is needed. No kill, no verification pass, no retries.

- Give the whole fallback its own short deadline, about 2 seconds. Keep it short on purpose: a successor supervisor
  waits for its predecessor for the stop budget plus a margin (search `core.rs` for `STATE_DIR_CLAIM_WAIT`), so a long
  fallback would make the successor give up first. Do not change the existing 10-second budget.
- Best effort: a failure or a hung tmux just ends the attempt, logged once; the process still exits.
- Test: call the fallback directly against a real `tmux -C` client on a private test server and assert that client's
  no-output flag is set afterwards, rather than driving a full shutdown under a contended lock. Follow the existing tmux
  test helpers in the supervisor crate.

**Spec principle.** SPEC_impl.md describes the acknowledged no-output step where it covers reconnect handoff (search for
"part of the handoff contract"; the sentence goes on "not cleanup polish"); planned-stop teardown has no paragraph of
its own. Add the principle there, or in a short new paragraph beside it if that reads better: defending against the tmux
abort on an abrupt close of an output-bearing control client (observed on distro tmux 3.6 and tmux 3.7b) is not worth
significant complexity unless an abort is observed on tmux 3.7c or later; without that evidence, findings that a rare
path can bypass the safe-teardown discipline are not reasons for further design or code, and the existing discipline
stays as it is. Name the evidence that would reopen the question. Tone down the "not cleanup polish" framing so it no
longer invites unbounded defense, without weakening what the existing code already does. Mention the bounded expiry
fallback where the planned-stop budget is described, if SPEC_impl describes it anywhere.

**Changelog.** `fix:` most likely, with `kind: fixed` if you can say something true and useful to someone running
Farhelm, otherwise `kind: none` with the reason (the consequence is unverified on the pinned tmux).

### Hard complexity bound

The user's bound: the code change stays small, roughly 150 lines including the test. If, once you understand the code,
the fallback cannot be done within that (or needs anything beyond what this outline describes, such as a new roster, a
second budget, changes to the lock, or restructuring `shutdown_output_clients`), do not grow it. Instead land only the
spec part in the PR: narrow `review_feedback_queue/shutdown-expiry.md` to the code part (with a note of what you found
that made it exceed the bound) rather than removing it, keep its index line, and record in the ledger's Execution field
that the spec part is complete and the code part was left, with the reason. Say so plainly in the report. This is the
agreed fallback; it is not a block.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` and
`cargo clippy -p farhelm --bins -- -D warnings` (the supervisor's `test-seams` feature differs between them), and
focused nextest selections for the touched supervisor modules through `scripts/record-test-run.py` with
`--tmux required` and the pinned tmux (`docs/test-run-evidence.md`). Any PR that changes Rust tests: run
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.
`dprint check` on the changed Markdown.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-shutdown-expiry-quiesce-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution field on the PR) rather than starting over. If it does not exist, this is a fresh
start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
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

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/shutdown-expiry-quiesce/01-shutdown-expiry-quiesce`.
- One outcome, one commit, one bookmark, one draft PR, per root `AGENTS.md` (Execute triage outcomes). Within this run,
  if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an earlier
  run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- The PR removes the feedback file `review_feedback_queue/shutdown-expiry.md` and its line in
  `review_feedback_queue/INDEX.md` (or narrows them, where this file says so), and updates its own `TRIAGE_OUTCOMES.md`
  Execution field to `complete` with the jj change ID, bookmark and PR URL. Record the change ID and bookmark before
  creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, if it changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
its changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: the `TRIAGE_OUTCOMES.md` entry, the outline above, and the user's
decisions. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as
root `AGENTS.md` requires. Also give the reviewer the hard complexity bound and ask it to flag anything that grows the
change beyond it. Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch
command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a client roster, a second or longer budget, changes
to the attachments lock, restructuring `shutdown_output_clients`, any new defense against the tmux abort beyond the
expiry fallback; these are examples, not a blacklist), and whenever the same component has needed repeated corrective
review rounds, run a fresh-context review through galaxy-brain with this charter, supplying the ledger entry, the user's
decisions above, this outline, the current diff and the proposed departure (what changed, why it is necessary, and which
simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the fallback's deadline, where the spec principle went and its wording, and
whether the hard complexity bound held, the Conventional Commit type and changelog kind, and every review finding you
decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. The hard complexity bound above has an agreed fallback (land only the spec part); use it rather than
blocking. If the work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one
plan, step 10). If current code or specs have moved so that the recorded decision no longer applies, that is a question
for the user too, per root `AGENTS.md` (Execute triage outcomes); never re-triage the item yourself.

## Done criterion

The plan is complete when its one draft PR exists, meets the acceptance criteria above, and (if it changes code, tests
or scripts) passed the review gate, with its `TRIAGE_OUTCOMES.md` Execution field updated and its queue item removed or
narrowed as this file says. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through
the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
