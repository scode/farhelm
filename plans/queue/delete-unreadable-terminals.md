# Delete keeps acting on what it knew after a terminal stops responding

Written against main at d81a3980 on 2026-11-04. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Execute three triage outcomes recorded on 2026-11-04, all `fix code`, per
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes" (one PR per outcome). Their
`TRIAGE_OUTCOMES.md` entries are the binding statement of what to do; read each in full from `main@origin`, together
with its feedback file under `review_feedback_queue/`:

1. `retained-delete-approval-can-kill-new-tab.md` (recorded without asking under the clearly-settled rule)
2. `hidden-live-tmux-tabs-escape-delete-consent.md`
3. `failed-tab-recovery-loses-delete-child-root.md`

All three are in Delete's live-work guard and its process sweep in the supervisor, so they share a stack.

### What each PR does

1. **Last-moment check missing a new tab.** `still_alive_for_delete` (`crates/farhelm-supervisor/src/service/core.rs`,
   around 14662-14760) calls `hosts.untold()` (`service/session_hosts.rs`, around 587-607), which refuses only for a tab
   unreadable at that moment, then after a tmux round trip calls `hosts.live_tabs()` (around 639-668), which counts only
   readable live tabs. A tab that goes unreadable between the two reads is missed by both. Decide from one reading of
   each tab's liveness, or recheck "cannot tell" after counting, so a tab unreadable at any point while the guard runs
   makes a nothing-alive Delete refuse (SPEC.md "Lifecycle operations", Confirmed 2026-10-01). A unit test flips a tab
   between the two reads. `fix:` with a changelog fragment.
2. **Old tmux tabs vanishing.** After upgrading past 0.33, Restart moves the agent into a new terminal while old tmux
   tabs stay. With no agent terminal (its host lost, then a supervisor reload), the tab listing (`service/status.rs`,
   around 531-541) and Delete's guard (`core.rs` around 14735-14749, `None => 0`) find old tmux tabs only through the
   agent terminal, while teardown (`service/teardown.rs`, around 180-198 and 759-801) still ends the session's durable
   tmux session. Find the tabs by the session's durable tmux name, as teardown does (`store.tmux_name`), in both the
   guard and the listing; the listing path is synchronous, so the name may need to be on the entry. Test a session with
   old tmux tabs and no agent terminal. `fix:` with a changelog fragment.
3. **Mac stray process after Delete.** A failed automatic reconnect replaces an unreadable tab or agent terminal with a
   placeholder that drops the pinned program identity: `HostTab::after_retry` (`service/core/host_tabs.rs`, around
   236-243) through `unavailable_again`/`ended()` (`crates/farhelm-supervisor/src/hosts/terminal.rs`, around 1997-2031)
   for tabs, and the agent's failed mid-life retry in `service/core/host_handover.rs` (around 1550-1639) through
   `HostTerminal::unavailable`. Delete's `sweep_roots` (`session_hosts.rs`, around 510-547) then has no root for that
   program. Keep the identity (process number and start time) across the failed retry for both, and root the sweep at
   it, validated against the process table as `pinned_child_root` already does. A unit test covers a failed retry
   followed by Delete. `fix:` with a changelog fragment.

### Acceptance criteria

- Three PRs, each meeting its ledger entry's completion criteria (each with a 100-line gate: over it, block and bring it
  back), each recording its execution in the ledger, and each removing its feedback file and index line.
- No change to the terminal host's protocol (`crates/terminal-host/SPEC.md`, "Change policy").

## Decisions already made

- **The outcomes:** item 1 recorded under the 2026-10-19 clearly-settled rule; items 2 and 3 picked by the maintainer on
  2026-11-04 ("Fix both").
- **Grouping:** the maintainer approved these three as one plan on the 2026-11-04 planning page.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the three `TRIAGE_OUTCOMES.md` entries named above.

**Binding repository constraints:** SPEC.md "Lifecycle operations" (Confirmed 2026-10-01 destructive-confirmation rule;
process-tree ownership: "A server that stays in the foreground under the agent remains in the terminal's process tree
and is still reaped"); `crates/terminal-host/SPEC.md`; root `AGENTS.md`, "Code rules", "Tests and validation",
"Changelog fragments", "Conventional Commits"; `.agents/test-authoring.md`;
`review_feedback_queue/.agents/execute-outcomes.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): item 3 stores the identity on the
placeholder handle rather than in a new durable record.

## Outline

Three PRs, bottom to top:

1. `plan/delete-unreadable-terminals/01-guard-one-liveness-read`
2. `plan/delete-unreadable-terminals/02-old-tmux-tabs-by-session`
3. `plan/delete-unreadable-terminals/03-keep-child-root-on-failed-retry`

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy on `farhelm-supervisor`
with `--all-targets`, and through `python3 scripts/record-test-run.py` with the pinned tmux: the "host terminal switch
points" selection (Delete's handling of hosts), "terminal tabs in terminal hosts", and "moving old sessions into
terminal hosts" for item 2; `python -B scripts/check-test-sleeps.py` when tests change.

## Fallbacks

Examples of departures that need the scope reassessment: a new durable record of child identities, changing Delete's
confirmation wording, or touching the terminal host's protocol.

Decisions to log: where item 2's tmux name comes from on the listing path, where item 3 keeps the identity, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-delete-unreadable-terminals-log.md` in the parent directory of the checkout you run in, derived as that
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
  `plan/delete-unreadable-terminals/<nn>-<short-name>`, as the outline names them.
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

Blocked on 2026-11-05 (claim 3e6ee9).

### Keeping old tabs visible needs a larger fix

After an upgrade moves an agent into a new terminal, its old terminal tabs keep running. If the agent's terminal is
later lost, those tabs can disappear from the session list even though Delete still ends them. Delete can therefore stop
running tabs after showing a confirmation that says nothing is alive.

The agreed second outcome fixes both the missing tabs and Delete's live-work check, with a gate of about 100 changed
lines of code, tests and documentation. The smallest coherent proposal found is 164 lines, excluding the ledger, queue
removal and changelog. The session list and the replies to Rename and Restart must all obtain and show the surviving
tabs; Delete must count them. An existing test explicitly requires skipping that lookup and must be corrected along with
the comments describing it. A smaller guard-only change would leave the required visibility repair incomplete. The
proposal uses the existing session identity and adds no persistent metadata or terminal-host protocol change.

Allow this larger, roughly 164-line single PR, or retain the current limit and leave the second outcome pending. I
recommend allowing the complete proposal. It has been inspected and sized privately, but has not been implemented,
compiled or correctness-reviewed.

The independent first and third outcomes are in draft PRs: [#3024](https://github.com/scode/farhelm/pull/3024/changes)
refuses Delete when a tab becomes unreadable during its final check, and
[#3032](https://github.com/scode/farhelm/pull/3032/changes) preserves program cleanup after a failed agent or tab
reconnect. Both passed the required independent review and targeted checks. The third is temporarily based on the first;
an authorized second fix must be inserted between them before this plan can be delivered. The plan remains incomplete.
