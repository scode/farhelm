# Fix takeover hint when an agent asks to add a host owned by another Farhelm

Written against main at a5157ee92 on 2026-11-20. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **An agent asking to add a host another Farhelm owns is told to do something impossible.** An agent running in a
> session can ask Farhelm to add a remote host (you approve it). Since a host now belongs to exactly one Farhelm
> installation, adding a host that another one already manages (another machine's Farhelm, or the installed app when the
> request goes to a devbuild) is refused with "Another Farhelm (...) manages this host. Use Take over in Hosts first."
> But Take over is an action on the host's row in Hosts, and this host has no row there, because adding it is exactly
> what was refused. The message should say what works instead: add the host yourself with Add host, which asks whether
> to take it over from the other Farhelm. Raised by the one-helm-per-supervisor landing, 2026-11-19.

### Where it stands (from planning, at main a5157ee92)

- In `crates/farhelm-helm/src/provisioning/service.rs`, `owner_refusal` (lines 416–421) returns
  `"Another Farhelm ({}) manages this host. Use Take over in Hosts first."`.
- When probing a host (`ProbeObservation::OwnedSupervisor`, lines 1017–1035), one `if` statement currently collapses
  three distinct refusal cases onto `owner_refusal(&owner)`:
  1. `!matches!(&registration, ProbeRegistration::Ssh { new_entry: Some(_), .. })` — an already-registered host (where a
     row exists in Hosts and `"Use Take over in Hosts first."` is accurate).
  2. `host_identity.is_none()` — a new-host add where the remote supervisor reported no install identity (so manual Add
     host cannot take it over either).
  3. `from_agent` — an agent asking to add a new host (where no row exists in Hosts, and adding the host yourself in
     **Add host** is what offers Take over).
- `crates/farhelm-helm/src/provisioning.rs` (`takeover_discovery_waits_for_one_user_confirmation`, lines 4596–4628)
  exercises the `from_agent` refusal, but line 4625 currently asserts only `message.contains("ab12")`.
- `crates/gui/farhelm-gui/src/hosts/add.rs:1215-1217` has a fallback arm for `Reply::TakeOver` when `self.from_agent` is
  set; because the helm returns `Refused(...)`, that fallback does not fire in normal same-build flow and needs no GUI
  plumbing change.

### Acceptance criteria

1. **Accurate refusal wording per case.** In `crates/farhelm-helm/src/provisioning/service.rs`
   (`ProbeObservation::OwnedSupervisor`):
   - Already-registered host (`!matches!(&registration, ProbeRegistration::Ssh { new_entry: Some(_), .. })`): keep
     `"Another Farhelm (<tag>) manages this host. Use Take over in Hosts first."` unchanged.
   - New-entry probe whose supervisor reported no install identity (`host_identity.is_none()`): refuse with a message
     naming the owning Farhelm tag and stating that it cannot be taken over because the host reported no install
     identity (checked before `from_agent`, so even an agent request against an identityless host is not told to take it
     over via Add host).
   - Agent-requested new-host add (`from_agent` with `host_identity.is_some()`): refuse with
     `"Another Farhelm (<tag>) manages this host. Add it yourself with Add host to take it over."`
2. **Existing already-listed refusals unchanged.** Setup, Update, uninstall, and connection refusals for hosts already
   in Hosts keep `"Use Take over in Hosts first."` (and their existing wording).
3. **Tests.** Update `takeover_discovery_waits_for_one_user_confirmation` in `crates/farhelm-helm/src/provisioning.rs`
   to assert the new Add-host hint wording (and that it does not mention `"Use Take over in Hosts"`), and add tests
   covering the missing-identity new-entry branch (both user and agent) and the already-registered host branch, per
   `.agents/test-authoring.md`.
4. **Docs and cleanup.** Check `SPEC.md` and `SPEC_impl.md` for any mention of the agent host-add ownership refusal
   wording and update if needed. Remove this plan's TODO.md entry in the PR and leave a changelog fragment.

## Decisions already made

Settled with the maintainer on the 2026-11-20 planning page:

- **Wording:** For an agent-requested host add owned by another Farhelm, say
  `"Another Farhelm (<tag>) manages this host. Add it yourself with Add host to take it over."`; if a new host reports
  no install identity, say it cannot be taken over because it reported no identity; keep
  `"Use Take over in Hosts first."` for hosts already listed in Hosts.
- **Review gate:** A single fresh-context `gpt-6.2-sol` agent at `high` effort with the general review charter.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** The plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** The TODO entry and the decisions above.

**Binding repository constraints:**

- `SPEC.md` ("Topology" / Take over, around lines 445–450, and "Agent-spawned sessions" / host-add requests).
- `.agents/test-authoring.md` for every test added or changed, and `scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation".

**Planner proposals** (verified against the code and planning review):

- **Split the three branches in `service.rs:1024-1035` in precedence order:** check `!is_new_ssh_entry` first, then
  `host_identity.is_none()`, then `from_agent`. Keep `owner_refusal` untouched for all already-registered host callers.
- **No GUI changes needed:** `crates/gui/farhelm-gui/src/hosts/add.rs` already displays
  `ProvisioningRequestError::Refused` directly as the helm error message.

## Outline

One PR:

1. `plan/agent-add-host-takeover-hint/01-agent-add-host-refusal` (`fix:`): split the `OwnedSupervisor` refusal branches
   in `crates/farhelm-helm/src/provisioning/service.rs`, update/add provisioning tests, check and update `SPEC.md` /
   `SPEC_impl.md` if affected, add a changelog fragment, and remove the TODO.md entry.

## Fallbacks

- If any agent-request test or CLI test pins the old `"Use Take over in Hosts first."` text on the agent host-add path,
  update those assertions to the new wording and log it.
- Decisions to log: the exact phrasing of the missing-identity takeover refusal, and every review finding declined with
  its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-agent-add-host-takeover-hint-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-agent-add-host-takeover-hint-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmark is
  `plan/agent-add-host-takeover-hint/01-agent-add-host-refusal`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves or extends the fragment its type calls for, describing in user-facing words
  what was built. Where what got built departs from what this file describes, the fragment's context paragraphs say how
  (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's TODO.md entry.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing this plan's TODO.md
entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
