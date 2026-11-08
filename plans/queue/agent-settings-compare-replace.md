# Tensorlake hosts: never overwrite an agent settings file that changed after Farhelm read it

Written against main at 78fd90f3 on 2026-11-07. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute one triage outcome recorded on 2026-11-07, per `review_feedback_queue/.agents/execute-outcomes.md`, "Execute
triage outcomes" (one PR; a `fix spec+code` outcome is never split). Its `TRIAGE_OUTCOMES.md` entry is the binding
statement of what to do; read it in full from `main@origin`, together with its feedback file under
`review_feedback_queue/`:

1. `agent-settings-save-overwrites-concurrent-editor.md` (`fix spec+code`)

### What the PR does

When Farhelm delivers recipe credentials into a Tensorlake host's Claude Code or Codex settings file (at setup, on a
connection whose remembered delivery differs, and on every credential replacement while connected), it reads the file,
merges its keys on the helm, and replaces the whole file about a second later
(`crates/farhelm-helm/src/provisioning/credential_delivery.rs`, around 456-493; the install in `backend.rs`
`install_source`, around 991-1125, compares the destination only with the new bytes). Anything saved in between, usually
by a running agent, is silently undone, and an agent's own save can likewise undo Farhelm's new token (that reverse
direction is not part of this outcome; mention it in the report).

The final replace checks that the destination still matches the bytes that were read (or is still absent) and refuses
otherwise. On a mismatch, delivery re-reads and merges once; if the file changed again, it falls back to the existing
amber warning and retries on the next connection, without recording the delivery as done. One SPEC_impl.md sentence
states the rule beside the delivery description. Tests cover a change between read and replace for an existing file and
for an absent one. 100-line gate (estimated 50-90): over it, block and bring it back. `fix:` with a changelog fragment.

### Acceptance criteria

- One PR meeting the ledger entry's completion criteria, recording its execution in the ledger, and removing the
  feedback file and index line.

## Decisions already made

- **The outcome:** the maintainer picked "Refuse to replace a changed file" on the 2026-11-07 triage page. Principle:
  Farhelm's delivery into an agent settings file never undoes a change saved to that file after Farhelm read it. The
  2026-10-21 last-write-wins exception (SPEC.md "One GUI at a time") does not extend to this.
- **Grouping:** its own plan, approved on the 2026-11-07 planning page.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the `TRIAGE_OUTCOMES.md` entry named above.

**Binding repository constraints:** SPEC.md's Tensorlake setup paragraph ("Unrelated settings are preserved") and "One
GUI at a time"; SPEC_impl.md "Adding Tensorlake hosts" and the credential delivery description; root `AGENTS.md`, "Tests
and validation", "Changelog fragments", "Conventional Commits"; `.agents/test-authoring.md`;
`review_feedback_queue/.agents/execute-outcomes.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): pass a hash of the bytes read (or
"absent") to the finishing command on the host, which compares and refuses, so the check and the `mv` happen together on
the host rather than in a separate round trip. `install_source` already guards its finishing command this way for its
setup-ownership check (`refuse_setup_managed`, around 1094-1118).

## Outline

One PR: `plan/agent-settings-compare-replace/01-compare-before-replace`.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-helm`, the credential delivery and provisioning backend unit tests through
`python3 scripts/record-test-run.py`, and `dprint check` on the spec file. No cloud calls.

## Fallbacks

Other Tensorlake plans were in flight or queued when this was written (`tensorlake-credentials-polish.md`,
`recipe-bases.md`, `prepared-snapshots.md`, `sandbox-sessions.md`). If they have landed and moved the code this file
names, follow the current code and correct this file's names and line numbers to the truth, logged as a DECISION; if
they changed the behavior an outcome is about, so that the ledger's completion criteria no longer make sense, that is a
block.

Examples of departures that need the scope reassessment: file locking on the host, a new protocol between helm and host,
or retrying more than once.

Decisions to log: where the comparison runs, the retry shape, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-agent-settings-compare-replace-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/agent-settings-compare-replace/<nn>-<short-name>`, as the outline names them.
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

## Blocked

Blocked on 2026-11-08 (claim 6f519f).

### Credential delivery needs a larger fix than the agreed size limit

When Farhelm delivers a recipe's credentials to a Tensorlake host, it can overwrite a Claude Code or Codex settings edit
saved after Farhelm read that file. The agreed fix checks the destination immediately before replacing it, re-reads and
merges once if it changed, and leaves the existing amber warning if that second attempt also conflicts. The plan
requires blocking if code, tests and documentation exceed about 100 changed lines.

Source inspection and an independent scope review found that the existing installer has no old-content or
expected-absence input. It also serves supervisor units, Codex login files and GitHub tokens, through production and
test backends. The smallest complete change needs that input carried to the final replacement command, a distinct
changed-file result, the one-retry merge, and tests for an existing file changing and an absent file appearing. Tests
must also establish that a second conflict preserves the user's file and does not record delivery as complete.

A smaller-looking check before uploading would leave the same overwrite window open. Removing the retry or its tests
would omit the agreed behavior. The bounded approach can reuse the installer, its temporary cleanup, existing warning
and successful-delivery records; it needs no new lock, protocol or background task. The source-based estimate is 180–280
added plus removed lines, including tests and documentation, rather than the original 50–90. No exact diff has been
measured. No code has been changed, no PR opened and no runtime checks run while this gate is unresolved.

Allow up to about 280 changed lines for this same implementation, retaining the one retry and the existing warning
behavior, or keep the 100-line limit and defer this fix? I recommend the larger allowance with the same scope and
required independent code review. Deferring leaves the concurrent-edit overwrite possible. An agent saving its own stale
settings after Farhelm's successful delivery is a separate direction of the race and remains outside this fix.
