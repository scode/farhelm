# Name the program a Stop or Delete approval card would end

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Name the program a Stop or Delete approval would end.** An agent can request Stop or Delete of a Bash or Zsh
> session, but the approval card says it would stop an agent. The card in `approvals/model.rs` needs the resolved
> target's launch kind from `ApprovalSession`. Carry that fact compatibly and use program-neutral wording if an older
> helm cannot supply it. Say which processes the action would stop, including terminal tabs where the deletion guard
> permits it. Keep the approved deletion tied to the same target and guard. Test shell targets in both actions and check
> the approval wording with and without live processes.

In product terms: carry the resolved target session's program kind compatibly on `ApprovalSession` (no supervisor
protocol bump needed), populate Delete's target directly from the same live snapshot used to compute its deletion guard
(and query live session on cache miss for Stop), and update Stop and Delete approval card wording in
`approvals/model.rs` so Bash and Zsh targets say `"shell"` (matching ordinary Stop/Delete dialogs), agent targets say
`"agent"`, and command targets or older helms omitting the field use program-neutral `"main program"` wording.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Proto wire type (`crates/farhelm-proto/src/approvals.rs:84-88`, `session_launch.rs:234, 364`):**
  - `ApprovalSession { id, title, host_name }` has no `deny_unknown_fields`. Add an optional, backward-compatible field
    `#[serde(default, skip_serializing_if = "Option::is_none")] pub launch_kind: Option<ApprovalLaunchKind>` (compact
    enum: `Agent`, `Shell`, `Command`, `Legacy`, plus `#[serde(other)] Unknown` for forward compatibility — never
    leaking raw command text). No `PROTOCOL_VERSION` bump is needed because `ApprovalSession` is strictly on the helm ↔
    GUI REST / UI-channel contract.
  - Define `ApprovalLaunchKind::of(launch: &SessionLaunch)` reusing existing `SessionLaunch::is_shell()` (first) and
    `SessionLaunch::agent_type().is_some()` (second, covering both `SessionLaunch::Agent` and `SessionLaunch::Command`
    with `agent: Some(_)`), then `Command` / `Legacy`.
- **Helm producers (`crates/farhelm-helm/src/approvals.rs:643-668`, `agent_requests.rs:605-655, 2792-2848`):**
  - Extract a single helper `describe_session_with_info(state, key, entry_uuid, info: Option<&SessionInfo>)` in
    `crates/farhelm-helm/src/approvals.rs` used by `describe_session`, `agent_delete`, and Stop's cache-miss path (no
    duplicated `SessionRef` formatting or `list_hosts()` lookup).
  - `agent_delete` (`agent_requests.rs:2792-2830`) already calls `client.list_sessions().await` to obtain `shown` and
    compute `guard = agent_delete_guard(shown, stop_if_running)`, then discards `shown` and calls `describe_session`.
    Build the target `ApprovalSession` directly from `Some(shown)` via `describe_session_with_info` so the card's target
    kind and title match the exact live snapshot bound to `guard`. Also update `agent_delete_guard`'s CLI refusal text
    (`agent_requests.rs:2842`) to name `"shell"` / `"main program"` accurately for non-agent targets (and update any
    `agent_requests.rs` tests pinning the old `"agent or terminal tabs"` refusal wording).
  - `ParsedAgentVerb::Stop` (`agent_requests.rs:605-655`): keep heading-only (no new liveness guard); populate
    `launch_kind` from `cached_session`, falling back to a **best-effort** `client.list_sessions().await` on cache miss
    (reusing `client` already returned by `route_active_session` at line 619) so a freshly created Bash/Zsh session gets
    accurate shell wording, while falling back to `launch_kind: None` (`"main program"`) if `list_sessions()` errors or
    omits the session rather than turning a metadata lookup failure into a new Stop refusal.
- **GUI approval card model (`crates/gui/farhelm-gui/src/approvals/model.rs:229-270, 509-548`):**
  - Keep approval card wording separate from `session_list/stop.rs::program_word` (ordinary dialogs fall back to
    `"agent"`, whereas approval cards fall back to `"main program"`).
  - `summary(action)` (`ApprovalAction::Stop { target, .. }`):
    - `Shell` -> `"Stop a session's shell and close its terminal tabs"`
    - `Agent` -> `"Stop a session's agent and close its terminal tabs"`
    - `Command | Legacy | Unknown | None` -> `"Stop a session's main program and close its terminal tabs"`
  - `facts(action)` (`ApprovalAction::Delete { target, guard, .. }`): `"what deletion stops"` substitutes `"shell"`,
    `"agent"`, or `"main program"` across all three `DeleteGuard` arms (`NothingAlive`, `AgentEnded`, `Unconditional`),
    preserving exact mention of terminal tabs per the guard.
- **Tests & struct literals:**
  - `crates/farhelm-proto/src/approvals.rs` wire compat round-trip tests (missing field -> `None`, unknown variant ->
    `Unknown`) and `ApprovalLaunchKind::of` tests.
  - `crates/farhelm-helm/src/agent_requests.rs`: Stop and Delete tests for Bash/Zsh shell targets with and without live
    processes, verifying Delete stays bound to the exact snapshot and guard, and verifying Stop cache-miss best-effort
    lookup and error fallback.
  - Update `ApprovalSession` struct literals across `crates/farhelm-helm-client/src/collections.rs` and `farhelm-gui`
    (`approvals/{model_tests,region_tests,card_tests,answers_tests,host_add_tests,tunnel_tests}.rs`,
    `goldens/{approvals,hosts}.rs`, `sounds/owner/tests.rs`). In particular update
    `crates/gui/farhelm-gui/src/approvals/model_tests.rs:729-751, 1427-1455, 1624-1644` (including the `WholeCard` table
    test) and `region_tests.rs:255`, and add explicit shell + fallback wording tests for both Stop and Delete across all
    three `DeleteGuard` states. (Existing approval golden scenes render only `Launch`, `TemplateWrite`, and `HostAdd`,
    so no golden pixels change.)

### Acceptance criteria

1. `ApprovalSession` carries `launch_kind: Option<ApprovalLaunchKind>` compatibly (omitted when `None`, unknown variants
   deserialize safely, no `PROTOCOL_VERSION` bump), derived via `SessionLaunch::is_shell()` / `agent_type()`.
2. Helm populates `launch_kind` for Stop (best-effort live lookup on cache miss, falling back to `None` on error) and
   Delete (built directly from the same live `SessionInfo` snapshot bound to `DeleteGuard`, keeping approved deletion
   tied to the same target and guard).
3. Stop approval headings and waiting-list headers say `"shell"` for Bash/Zsh targets, `"agent"` for agent targets, and
   `"main program"` for command/legacy/unknown/older-helm targets. Stop stays heading-only (no new liveness fact).
4. Delete approval `"What deletion stops"` lines say `"shell"`, `"agent"`, or `"main program"` accurately across all
   three `DeleteGuard` states (`NothingAlive`, `AgentEnded`, `Unconditional`), accurately naming terminal tabs where the
   guard permits stopping them.
5. Tests cover shell targets in both Stop and Delete (with and without live processes), older-helm fallback (`None`),
   and Delete guard refusal wording; the TODO.md entry is removed.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Name Bash and Zsh targets generically as `"shell"` on Stop and Delete approval cards (matching ordinary Stop/Delete
  dialogs in the session list).
- Keep Stop approval cards heading-only (query live session only on cache miss to obtain its accurate `launch_kind`,
  without adding a new `"What stop ends"` live-process fact).
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `SPEC.md:70-72, 4351-4360`.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Use a small `ApprovalLaunchKind` enum (`Agent`, `Shell`, `Command`, `Legacy`, `#[serde(other)] Unknown`) on
  `ApprovalSession` so command strings are never duplicated onto cards that shouldn't carry them.
- Deliver in **1 cohesive PR** (~70 lines of production code across proto, helm, and GUI) so adding `pub launch_kind` to
  `ApprovalSession` does not churn `farhelm-gui` test struct literals twice across two PRs.

## Outline

1. `plan/approval-card-program-wording/01-name-program-in-stop-delete-approvals`: add `ApprovalLaunchKind` (derived via
   `SessionLaunch::is_shell()` / `agent_type()`) and optional `launch_kind` on `ApprovalSession`
   (`farhelm-proto/src/approvals.rs`), extract `describe_session_with_info` in `farhelm-helm/src/approvals.rs` and
   populate Stop (best-effort live lookup on cache miss) and Delete (bound to live `shown` snapshot + updated
   `agent_delete_guard` refusal wording) in `agent_requests.rs`, update `approvals/model.rs` wording for `Shell`,
   `Agent`, and `"main program"`, update struct literals and tests across `farhelm-proto`, `farhelm-helm`,
   `farhelm-helm-client`, and `farhelm-gui` (including `model_tests.rs:1624-1644`), add a `kind: fixed` changelog
   fragment, and remove the TODO.md entry ("Name the program a Stop or Delete approval would end").
   `fix: name the shell or program in Stop and Delete approval cards`.

## Fallbacks

- None expected; if any approval golden scene unexpectedly changes, refresh and publish it with `check-gui-goldens.py`
  - `gui-goldens.py publish` (blocking if publish cannot reach the private goldens storage repository).

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-approval-card-program-wording-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-approval-card-program-wording-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted tests in `farhelm-proto` (`approvals`), `farhelm-helm` (`agent_requests`, `approvals`),
`farhelm-helm-client` (`collections`), and `farhelm-gui` (`approvals`), `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `dprint check` on changed Markdown, and
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to
every test you add or change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Name the program a Stop or Delete approval would
  end"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

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
