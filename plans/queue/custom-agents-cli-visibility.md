# Make registered custom agents visible and consistent in the `farhelm agent` CLI

Written against main at fea806bc on 2026-11-23. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Make registered custom agents visible and consistent in the `farhelm agent` CLI.** Agents currently have no way to
> discover registered custom agent IDs through the CLI. Make custom agents behave like built-in agents in the CLI so
> they are visible to agents as usual: add a way to list registered custom agents (and include them alongside built-in
> types in unknown-agent refusals), reconcile the `--conversation` + `--recipe` disagreement between CLI and templates,
> and restore listing valid built-in choices when `--effort` is mistyped for a built-in agent.

In product terms, sharpened by the maintainer's planning decisions: add `farhelm agent agents [--json]` listing both
built-in agent types and registered custom agents (withholding configured command text, matching
`farhelm agent templates`), list registered custom agent IDs alongside built-in types in unknown-agent refusals across
both session creates and template writes, forbid `--conversation` with a recipe everywhere (both `--recipe` and recipe
templates), and restore listing valid built-in effort choices in `--effort` refusals for built-in agents.

### Acceptance criteria

1. **`farhelm agent agents [--json]` discovery:**
   - Adds `farhelm agent agents [--json]` backed by `AgentVerb::Agents {}` /
     `AgentReply::Agents { agents, caller_host_id }` (bumping `PROTOCOL_VERSION` and discovery JSON `schema_version`
     cleanly).
   - Lists every built-in agent type (in canonical `LaunchHarness::BUILT_INS` order) followed by every registered custom
     agent, using a dedicated listing struct that withholds custom command text and reports each entry's ID, kind
     (`built_in` / `custom`), display name, and launch capabilities (such as resumption and initial prompt support).
   - Accessible to confined environment origins alongside `Hosts` and `Sessions`, so confined agents can discover valid
     `--agent` values for `farhelm spawn` / same-host `farhelm agent create`.
   - Human table output rendered in `crates/farhelm/src/render.rs`, and `farhelm agent instructions` teaches the new
     command alongside `hosts`, `sessions`, and `templates`.
2. **Unknown-agent refusals list registered custom agents too:**
   - `crates/farhelm-helm/src/custom_agents.rs::unknown_agent(id, registered)` lists both built-in agent types and
     registered custom agent IDs (or notes none are registered).
   - Both `crates/farhelm-helm/src/agent_launch.rs:329` (`create`/`spawn`) AND
     `crates/farhelm-helm/src/agent_requests.rs:3315` (`template create`/`edit`) use `unknown_agent(id, &registered)` so
     a typo like `--agent cluade` on template writes no longer returns the misleading stale-registration message.
3. **`--conversation` forbidden with recipes everywhere:**
   - `crates/farhelm-helm/src/agent_requests.rs::create_for_agent` refuses `edits.conversation.is_some()` whenever a
     recipe template selects a new disposable environment (matching `--recipe`'s existing refusal).
   - Drops the now-unused `conversation` parameter from `launch_environment_for_agent` and
     `CreateReq::for_agent_environment` (leaving user-initiated GUI sandbox creates intact).
4. **Built-in `--effort` refusals list valid choices:**
   - In `crates/farhelm-proto/src/launcher.rs::check_template_shape` (stateless; do not add a catalog parameter), if
     `!agent.offers_effort()` state that the agent does not accept `--effort`; otherwise list `LaunchEffort::ALL`.
   - In `crates/farhelm-proto/src/launcher.rs::apply_template` (catalog-aware), if `!harness.offers_effort()` or
     `compatible_efforts(harness, next.model.as_deref(), context.catalog)` is empty, state that the agent/model does not
     accept `--effort`; otherwise list the compatible effort values.
5. **Specs, docs, changelog fragment, and TODO cleanup:** Update `SPEC.md`, `SPEC_impl.md`, and
   `website/src/content/docs/docs/agents/custom-agents.md` (if it describes CLI agent discovery), amend
   `releasing/changelog.d/custom-agents.md` if still present, and remove the covered entry from `TODO.md`.

## Decisions already made

### Maintainer decisions (binding)

- **Discovery subcommand:** Add `farhelm agent agents [--json]` listing both built-in and registered custom agents
  (withholding command text), plus list registered custom agents in unknown-agent refusals.
- **`--conversation` + recipe:** Forbid `--conversation` with both `--recipe` and recipe templates.
- **Built-in `--effort` refusals:** Restore listing valid built-in effort choices in the refusal.
- **Review gate:** Single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.`

### Binding repository constraints

- `AGENTS.md`, "Code rules": do not compare harnesses at call sites (`kind == X`, `matches!`, or `_` arms over
  harnesses). Reuse exhaustive methods on `LaunchHarness` (`BUILT_INS`, `offers_named_conversation`,
  `supports_initial_prompt`, `offers_model`, `offers_effort`) and add an exhaustive `pub const fn display_name(self)` on
  `LaunchHarness` in `crates/farhelm-proto/src/launch.rs` (noting it in
  `crates/farhelm-supervisor/src/agent_kind/mod.rs`'s module-doc map).
- `crates/farhelm-proto/src/lib.rs`: adding a new tagged `AgentVerb` / `AgentReply` variant crossing the supervisor
  relay requires bumping `PROTOCOL_VERSION`.
- `AGENTS.md`, "Changelog fragments": amending unreleased work edits its existing fragment
  (`releasing/changelog.d/custom-agents.md`) rather than adding a second one (note: a `PROTOCOL_VERSION` bump alone gets
  no public remote-hosts entry per `releasing/EDITORIAL_GUIDANCE.md`).

### Planner-chosen implementation shape

- Keep a dedicated small listing struct (`ListedAgent` / `AgentListingEntry`) on `AgentReply::Agents` rather than
  reusing `CustomAgent`, so `command` is structurally absent from the wire reply.
- Bump discovery JSON `schema_version` from 7 to 8 (matching how adding `templates` bumped 5 to 6 in
  `crates/farhelm/src/main.rs:2708`), keeping existing `hosts`, `sessions`, and `templates` shapes unchanged.
- Do not generalize `check_template_shape` to take a catalog; keep it stateless using `agent.offers_effort()` and
  `LaunchEffort::ALL`.

## Outline

### Existing mechanisms reused

- `crates/farhelm-proto/src/launch.rs` (`LaunchHarness::BUILT_INS`, `LaunchEffort::ALL`, capability methods).
- `crates/farhelm-proto/src/launcher.rs` (`check_template_shape`, `apply_template`, `compatible_efforts`).
- `crates/farhelm-proto/src/lib.rs` (`AgentVerb`, `AgentReply`, `PROTOCOL_VERSION`).
- `crates/farhelm-helm/src/custom_agents.rs` (`unknown_agent`), `crates/farhelm-helm/src/agent_launch.rs`,
  `crates/farhelm-helm/src/agent_requests.rs`, `crates/farhelm-helm/src/sessions.rs`.
- `crates/farhelm/src/main.rs`, `crates/farhelm/src/render.rs`, `crates/farhelm/src/agent_instructions.rs`.

### PR stack

1. **`plan/custom-agents-cli-visibility/01-refusals-and-recipe-conversation`** — Refusals and `--conversation` + recipe
   consistency (no wire-protocol bump):
   - Update `custom_agents::unknown_agent(id, registered)` to include registered custom agent IDs alongside built-in
     types, and route both `agent_launch.rs:329` and `agent_requests.rs:3315` (`template create`/`edit`) through it.
   - Restore listing valid built-in effort choices in `check_template_shape` and `apply_template` in
     `crates/farhelm-proto/src/launcher.rs`.
   - Refuse `edits.conversation.is_some()` for recipe-template creates in `agent_requests.rs`, and drop the unused
     `conversation` parameter from `launch_environment_for_agent` and `CreateReq::for_agent_environment`.
   - Add unit and e2e tests covering all three fixes.
2. **`plan/custom-agents-cli-visibility/02-agent-agents-discovery`** — `farhelm agent agents [--json]`, docs, and TODO
   removal:
   - Add exhaustive `LaunchHarness::display_name()` in `crates/farhelm-proto/src/launch.rs` (and note it in
     `crates/farhelm-supervisor/src/agent_kind/mod.rs`), add `AgentVerb::Agents {}` and `AgentReply::Agents` with a
     dedicated listing struct omitting `command`, and bump `PROTOCOL_VERSION`.
   - Handle `ParsedAgentVerb::Agents` in `crates/farhelm-helm/src/agent_requests.rs` (including for confined origins).
   - Wire `AgentCmd::Agents { json }` in `crates/farhelm/src/main.rs` (bumping discovery JSON `schema_version` to 8),
     add table rendering in `crates/farhelm/src/render.rs`, and update `crates/farhelm/src/agent_instructions.rs`.
   - Add unit and e2e tests (`agent_listing_real_stack.rs`, `agent_instructions`), update `SPEC.md`, `SPEC_impl.md`, and
     `website/src/content/docs/docs/agents/custom-agents.md`, amend `releasing/changelog.d/custom-agents.md` if still
     present, and remove the covered entry from `TODO.md`.

## Fallbacks

- If `releasing/changelog.d/custom-agents.md` was already consumed by a release before this plan runs, add new changelog
  fragments per `AGENTS.md`, "Changelog fragments".
- Scope-reassessment triggers (examples, not a blacklist): adding custom-agent write verbs
  (`farhelm agent custom-agent create/delete`), exposing raw command lines in agent discovery, or changing how
  `farhelm agent templates` renders.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-custom-agents-cli-visibility-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-custom-agents-cli-visibility-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Rebuild `farhelm-fixtures` before running e2e tests. Run targeted checks per PR:

- PR 1: `farhelm-proto` (`launcher`), `farhelm-helm` (`custom_agents`, `agent_launch`, `agent_requests`), `farhelm`
  (`structured_launches` / `agent_listing_real_stack` e2e), `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`.
- PR 2: `farhelm-proto`, `farhelm-supervisor` (`agent_kind` / protocol), `farhelm-helm` (`agent_requests`), `farhelm`
  (`agent_instructions`, `render`, `e2e` `agent_listing_real_stack`), `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `python3 releasing/check-changelog.py format`, `dprint check` on changed Markdown, `website` build if docs under
  `website/` changed, and `check-test-sleeps.py`.

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Make registered custom agents visible and
  consistent in the `farhelm agent` CLI."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
