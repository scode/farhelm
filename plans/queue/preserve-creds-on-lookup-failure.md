# Keep remote agent credentials intact when a recipe credential lookup fails

Written against main at fea806bc on 2026-11-23. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Keep remote agent credentials intact when a recipe credential lookup fails.** After #4679 combined Claude and Codex
> into one remote launch-credentials file, a failed lookup in the helm's SQLite credential catalog for either agent
> falls into the legacy-cleanup withholding path and rewrites or deletes the remote file without that agent's entry
> (even during an unrelated push such as a GitHub token update). Fail out and leave the existing on-disk credentials
> file untouched whenever either agent's SQLite lookup fails (keep per-agent withholding only for legacy settings
> cleanup, without trying to simultaneously update the other agent's entry in the same file), and name which agent
> failed in the host warning.

In product terms, sharpened by the maintainer's planning decisions: whenever looking up either Claude's or Codex's
recipe credential in the helm's SQLite catalog fails during delivery, skip updating the combined remote
`agent-credentials` file completely (leaving the existing file and both agents' stamps untouched), return agent-named
warning(s) ("Claude" and/or "Codex"), and still continue syncing independent slots (Codex `auth.json` login and GitHub
token) on that pass. When a credential push targets either Claude or Codex and the shared file write was skipped due to
a lookup failure on either agent, report the lookup failure as a push failure so a rotated token cannot falsely report
success.

### Acceptance criteria

1. **Preserve remote `agent-credentials` on any lookup failure:** In
   `crates/farhelm-helm/src/provisioning/credential_delivery.rs::apply_agent_environment()`, if `claude` is `Err` or
   `codex` is `Some(Err)`, collect all lookup failure warning(s) (naming "Claude" and/or "Codex") and return
   `Ok((guard, warnings))` immediately — without calling `retire_legacy_settings`, without writing or removing
   `<state_dir>/agent-credentials`, and without modifying `record.claude_environment` or `record.codex_environment`.
2. **Independent slots still sync; push failures are accurate:**
   - `apply()` still proceeds to sync `codex_login_credential` (`auth.json`) and `github_token_credential` after
     `apply_agent_environment()` returns lookup warning(s).
   - `push_one()` (`credential_delivery.rs:628-635`): because a lookup failure on either agent skips the entire shared
     `agent-credentials` write, when `uses_claude || uses_codex` and the shared write was skipped due to a lookup
     failure, `push_one()` includes the lookup warning(s) in `failure` (never silently reporting push success for a
     rotated Claude or Codex token that did not reach disk). When pushing an unrelated credential
     (`!uses_claude && !uses_codex`, e.g. GitHub token or Codex `auth.json` login), `failure` stays `None` while the
     host warning still surfaces the Claude/Codex lookup failure.
   - `setup()` (`DeliveryPhase::Setup`) still turns a Claude lookup warning into an `Err(...)`.
3. **Agent-named warnings and legacy-cleanup isolation:** `resolve_endpoint` names the affected agent ("Claude" or
   "Codex") in its lookup error, and `resolve_claude_login` names "Claude". Per-agent withholding remains intact
   strictly for `retire_legacy_settings` warnings.
4. **Tests, changelog fragment, and TODO cleanup:** Add targeted unit tests in `credential_delivery.rs` proving:
   - failed Claude or Codex lookup (or both) leaves `<state_dir>/agent-credentials` byte-for-byte untouched and stamps
     unchanged, including during a GitHub token update;
   - warnings name "Claude" / "Codex" (and both when both fail);
   - rotating Codex's endpoint token while Claude's lookup fails (and vice versa) reports a `DeliveryFailure` from
     `push_one()` rather than reporting success, while rotating a GitHub token succeeds with the host warning;
   - legacy-cleanup withholding still withholds only the affected agent. Amend
     `releasing/changelog.d/codex-recipe-launch-credentials.md` if still present, update `SPEC.md`/`SPEC_impl.md` if
     needed, and remove the covered entry from `TODO.md`.

## Decisions already made

### Maintainer decisions (binding)

- **Independent slots on lookup failure:** Skip only the combined `agent-credentials` write; still sync Codex login
  (`auth.json`) and GitHub token on that pass.
- **Agent-named warnings & legacy-cleanup boundary:** Name "Claude" / "Codex" in lookup failure warnings; keep per-agent
  withholding strictly for legacy settings cleanup.
- **Review gate:** Single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.`

### Binding repository constraints

- `AGENTS.md`, "Changelog fragments": amending unreleased work edits its existing fragment
  (`releasing/changelog.d/codex-recipe-launch-credentials.md`) rather than adding a second one.

### Planner-chosen implementation shape

- Track on `EndpointWarnings` whether the shared write was skipped due to a lookup failure (e.g. `lookup_aborted: bool`
  or distinct lookup vs cleanup fields) so `push_one()` reports a push failure whenever `uses_claude || uses_codex` and
  the shared write was skipped, while keeping per-agent filtering for legacy-cleanup withholding.

## Outline

### Existing mechanisms reused

- `crates/farhelm-helm/src/provisioning/credential_delivery.rs` (`apply`, `apply_agent_environment`, `push_one`,
  `resolve_endpoint`, `resolve_claude_login`, `EndpointWarnings`).

### PR stack

1. **`plan/preserve-creds-on-lookup-failure/01-preserve-on-lookup-fail`** — Preserve remote `agent-credentials` on
   lookup failure, name affected agent, and fix `push_one()` reporting:
   - Pass agent label ("Claude" / "Codex") to `resolve_endpoint` so warnings clearly name the agent.
   - Short-circuit `apply_agent_environment` before legacy cleanup or file write/remove whenever either lookup fails,
     collecting both warnings if both fail and marking `EndpointWarnings` as lookup-aborted.
   - Update `push_one()` so `uses_claude || uses_codex` treats a lookup-aborted `agent-credentials` pass as a push
     failure while independent GitHub/Codex-login pushes still succeed with the host warning.
   - Add targeted unit tests, amend `releasing/changelog.d/codex-recipe-launch-credentials.md` if still present, update
     `SPEC.md`/`SPEC_impl.md` if needed, and remove the covered entry from `TODO.md`.

## Fallbacks

- If `releasing/changelog.d/codex-recipe-launch-credentials.md` was already consumed by a release before this plan runs,
  add a new `kind: fixed` fragment per `AGENTS.md`, "Changelog fragments".
- Scope-reassessment triggers (examples, not a blacklist): splitting `<state_dir>/agent-credentials` back into two
  remote files, or altering the remote supervisor's credential loading format.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-preserve-creds-on-lookup-failure-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-preserve-creds-on-lookup-failure-watchdog.json`
beside it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the
agent scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- PR 1: `farhelm-helm` tests (`credential_delivery`), `cargo fmt --all -- --check`,
  `cargo clippy -p farhelm-helm --all-targets -- -D warnings`, `python3 releasing/check-changelog.py format`,
  `dprint check` on changed Markdown, and `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`.

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Keep remote agent credentials intact when a
  recipe credential lookup fails."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what was
  built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

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
alternatives considered: in particular those named under Planner-chosen implementation shape and Fallbacks above, and
every review finding you decided not to follow. The maintainer will ask for these later.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
