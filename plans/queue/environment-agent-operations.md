# Agent operations and CLI launches for sandbox and container sessions

Written against main at 231a66f7 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file records a change the maintainer approved; that change is yours to make in the spec.

## The goal

This plan runs after `podman-environments.md`, which itself runs after `sandbox-sessions.md`. Together they give a
session a disposable machine of its own (an **environment**): a Tensorlake sandbox or a rootless Podman container on one
of the user's hosts, reached as a hidden, session-owned host with Farhelm's supervisor inside, built from a shared
recipe. Both plans refuse every agent request from such a session (`farhelm spawn` and every `farhelm agent …` verb),
with a quiet notification on the session. Read both plan files (from git history if they have left `plans/queue/`),
their reports under `plans/reports/`, and the code they landed before starting; build on what actually landed. In
particular, `podman-environments.md`'s "Core abstractions" section describes the provider model (`ProviderKind`,
`EnvironmentProvider`, places, environment hosts) and its encoding rule in root `AGENTS.md`, "Code rules"; this plan
keeps to it.

This plan lifts the blanket refusal, for every provider alike, so that:

1. **An agent inside an environment can ask Farhelm to act, but only within its own environment**: act on sessions in
   that environment and spawn new sessions into it. It cannot reach other hosts, change templates or hosts, or create a
   new environment.
2. **Agents on ordinary hosts can launch a session in a new environment**, from the CLI (a flag naming a recipe and, for
   containers, the container host) or from a template that names an environment target, behind an approval card that
   always asks and doubles as the creation confirmation.

This plan covers these TODO.md entries (bucket "Near term"); read each in full in TODO.md:

- "Let agents in sandbox sessions ask Farhelm to act." (subsection "Tensorlake")
- "Containerized sessions: Farhelm integration inside the container." (`podman-environments.md` brings hooks,
  conversation capture and attachments by running a supervisor inside; what remains is the agent's requests)
- "Containerized sessions: `farhelm spawn` and CLI flags."
- The two approval-card findings from "Podman rigor: review findings for containerized sessions", which
  `podman-environments.md` hands to this plan: launch approval cards must show the environment target (reusing the
  template card's rows), and environment-bearing approval cards must use a card kind older pages cannot decode, as
  prompt-bearing and shortcut-moving cards already do. `podman-environments.md` keeps them in TODO.md as an entry of
  their own; this plan removes that entry.

The maintainer's words while planning (2026-11-08):

> agree on agent operations.

(agreeing that agents inside an environment act only within their own environment, and that creating a new environment
from inside one stays out.) The TODO entry's own guidance: an agent in a disposable sandbox "should not reach the user's
other hosts any further than it could from anywhere else (the trust questions in
`lore/2026-09-29-per-session-sandboxes-as-the-remote-host-goal.md` and SPEC.md's "Local authority and trust between
hosts")".

## Decisions already made

Settled with the maintainer on 2026-11-08, in conversation and on a planning page:

- **Scope for agents inside an environment:** only their own environment. Every provider alike; no per-provider rules.
- **Approval inside an environment:** the same approval rules as on any host, with the defaults a new host gets: the
  defaults sandbox-sessions and `podman-environments.md` already give environment hosts (those of a new Tensorlake
  host).
- **Agents on ordinary hosts may launch a session in a new environment**, behind an approval card that always asks
  (whatever the host's "without asking" settings say) and doubles as the creation confirmation, so a Tensorlake launch
  does not also show the billing confirmation separately. The card states what the user's own creation confirmation
  states (recipe, place, and for Tensorlake that it bills).
- **Agents inside an environment cannot create a new environment**, including spawning from the same recipe.
- **Environment lifetime with several sessions:** the environment stays until the last session in it is deleted (the
  maintainer's answer on the planning page). Most of the mechanism already exists: sandbox-sessions' 2026-11-09 decision
  (applied to containers by `podman-environments.md`) makes Delete remove the environment only with its last session,
  makes a non-final Delete remove only the chosen session and say the others and the environment remain, refuses a
  non-final Delete while the environment is suspended (paused) or unreachable, and has Suspend, Resume and Stop and
  suspend act on the shared environment with wording covering every session in it. Build on that landed rule; do not add
  a second ownership mechanism. What this plan changes: the multi-session case becomes normal instead of a
  failed-Replace exception; "creating ordinary additional sessions in an environment stays forbidden" is lifted for
  agent spawns and creates from inside that environment only; and the wording is adjusted ("The container stays: 2 other
  sessions use it"; the last Delete says it removes the container or sandbox and its files). The non-final Delete still
  refuses while suspended or unreachable.
- **Code reviewer:** a single fresh-context gpt-6.1-sol agent at high effort with the general charter below.
- **Opus consult before changing the core abstractions**, as in `podman-environments.md` (see "Changing the core
  abstractions").
- **No-workhorse mode**, the plans default.
- **Out of scope:** user-owned Tensorlake hosts requested by agents (TODO "Agent-requested Tensorlake hosts" stays),
  auto-suspend, anything provider-specific.

## The rules, in detail

Today the helm handles every agent request in `crates/farhelm-helm/src/agent_requests.rs`, `HelmAgentRequests::handle`,
with the request's origin (`AgentOrigin { host, connection }`) identifying the host the request came from. Placement for
`farhelm spawn` already goes to the origin host and refuses `--host`. sandbox-sessions put the refusal of requests from
session-owned hosts right after the verb is parsed, keyed on the origin host; `podman-environments.md` kept it
provider-neutral. Replace that refusal with these rules (a planner reading of the agreed scope; adjust to the landed
code and log a DECISION where it differs):

**From an environment host** (the origin is an environment host):

- **Spawn and create** place the new session on the origin host only: `farhelm spawn` as today; `farhelm agent create`
  with no host, the origin host, or a template that resolves to the origin host or carries no host is allowed; any other
  host, or a template naming another host or an environment target, is refused.
- **Clone** is always refused: cloning an environment session creates a new environment, which agents inside one may not
  do, and spawn or create onto the origin host already covers "another session here".
- **Lifecycle verbs** (rename, stop, restart, replace, delete, prompt, and the rest of today's set): only on sessions on
  the origin host. The existing self-delete and self-prompt refusals stay.
- **Listings** (hosts, sessions, and anything else that enumerates the fleet): filtered to the origin host. The host
  listing shows the agent's own environment only, under a name that does not leak other hosts.
- **Template verbs** (list, read, create, edit, delete) are all refused; the only use of a template is
  `create --template` as above, checked where the template is applied. **Host verbs** (add, edit, remove, setup) are
  refused.
- Anything else that reaches outside the environment (for example opening a URL in the user's browser) follows whatever
  SPEC.md says for agents on ordinary hosts, unless it would act on another host.
- Allowed requests go through the ordinary approval flow with the environment host's settings (see decisions).
- Refusals use one plain message ("Agents in sandbox and container sessions can only act within their own sandbox or
  container.", wording free) and keep the existing at-most-once-per-launch notification on the session.

**Aimed at an environment session from elsewhere:**

- **Delete** stays refused (only the user deletes an environment).
- **Clone** of an environment session now creates a new environment, so it goes through the always-asking environment
  approval card below instead of being refused.
- **Create, or a template, targeting an existing environment host** (for example `farhelm agent create --host` naming a
  hidden environment host, or a template whose host field holds one) is refused from anywhere other than that host
  itself: no agent puts a session into someone else's environment.
- Other verbs keep today's approval flow.

**Launching a new environment from an ordinary host:**

- `farhelm agent create` gains flags naming a recipe and, for a container, the container host (names free, for example
  `--recipe dev-box` and `--container-host build-box`; Tensorlake when no container host is given, or an explicit
  choice, whichever reads better against the existing flags). A template whose host is an environment target works
  through `--template`. `farhelm spawn` keeps placing on its origin host: this is a planner narrowing of the TODO entry
  "`farhelm spawn` and CLI flags" (spawn means "another session here"; launching elsewhere is `agent create`), so the
  report states it for the maintainer.
- These always produce an approval card, whatever "without asking" settings the origin host has. The card is a new card
  kind older pages cannot decode, shows the target with the template card's rows (recipe, place, provider, and for
  Tensorlake that it bills), and its approval is the creation confirmation.
- The helm then runs the same launch flow as a user's launch from the launcher (`podman-environments.md`), with its
  progress shown where agent-created sessions show theirs today.

## Requirement sources

**The user's request:** the quotes and decisions above.

**Binding repository constraints:**

- SPEC.md: "Agent-spawned sessions", "Local authority and trust between hosts", the approval-card paragraphs, the
  sandbox and container session paragraphs, the CLI reference for `farhelm spawn` and `farhelm agent`.
- SPEC_impl.md's agent-request and approval sections.
- Root `AGENTS.md`: "Code rules" (the harness rule and the provider encoding rule), "Tests and validation", "Changelog
  fragments", "TODO.md and FLAKES.md".
- `.agents/test-authoring.md`, `.agents/validation.md`, `.agents/test-evidence.md`.
- `website/AGENTS.md`, if the docs describe agent requests from sandbox or container sessions or the CLI flags (the
  approve-agent-requests and security-model pages are likely); name changed pages in the report.

**Planner proposals** (change them if the code says otherwise, and log a DECISION): the per-verb table above, the
listing filter's shape, the flag names, the refusal wording, and the card's layout.

## Acceptance criteria

- From a sandbox session and from a container session alike: spawn and create onto the own environment work through the
  approval flow; every refused case in "The rules, in detail" is refused with the stated message and at most one
  notification per launch; listings show only the own environment. Helm tests cover each verb family for both providers
  through the provider-neutral path (no per-provider branches in `agent_requests.rs`).
- With an agent-spawned second session in an environment: deleting the session that created the environment leaves the
  environment and the other session running, with the confirmation saying so (and refused while the environment is
  suspended or unreachable, as the landed rule says); deleting the last session removes the environment, confirmed
  against the provider; Suspend from either session says the other is frozen too. Covered for both providers through the
  provider-neutral path, reusing the landed multi-session rule rather than a new ownership mechanism.
- From an ordinary host: `farhelm agent create` with the new flags, and with a template naming an environment target,
  shows the new always-asking card and, once approved, launches the environment session; declining creates nothing.
  Clone of an environment session from elsewhere does the same. Delete aimed at an environment session from elsewhere is
  refused, and so is create or a template targeting an existing environment host from anywhere but that host. Clone from
  inside an environment is refused.
- An older page that cannot decode the new card kind does not show a half-understood card (the existing mechanism for
  prompt-bearing and shortcut-moving cards).
- SPEC.md and SPEC_impl.md are amended first, in the stack's first PR or a leading docs PR, for the agent-operation
  contract, the trust section, the new card kind and the CLI flags.
- Browser specs on the e2e fake cover the new card (both providers' wording) and the refusal notification. Rust e2e
  tests on the local Podman cover an agent inside a container spawning into it and being refused outside it.
- A changelog fragment (`kind: added`): agents in sandbox and container sessions can now ask Farhelm to act within their
  own environment, and agents elsewhere can launch sessions in new environments after an always-asked approval.
- The last PR removes the TODO.md entries this plan covers (listed under "The goal").

## Outline

The code today, from read-only maps made while planning (verify each; the two earlier plans will have changed some):

- **Client:** `crates/farhelm/src/agent_client.rs` (`SessionEnv`, `spawn_session`), CLI definitions in
  `crates/farhelm/src/main.rs` (`farhelm spawn`, `farhelm agent create/clone/template`, `LaunchFlags`).
- **Supervisor relay:** `crates/farhelm-supervisor/src/service/handlers.rs` (`handle_restricted_control`) and
  `service/agent_relay.rs` (`relay_agent_request`), notifications in `service/notifications.rs`.
- **Helm:** `crates/farhelm-helm/src/agent_requests.rs` (`AgentOrigin`, `handle`, `resolve_target`,
  `resolve_agent_create`, `clone_for_agent`, `merge_template_fields`), approvals in
  `crates/farhelm-helm/src/approvals.rs` and the UI's `crates/farhelm-ui/src/approvals.rs`.

A likely stack (reshape freely): (1) `docs:` spec amendments; (2) `feat:` the scoped rules for requests from environment
hosts, replacing the blanket refusal, with the listing filter and the multi-session wording on top of the landed
lifetime rule; (3) `feat:` the always-asking environment approval card and the CLI flags; (4) browser specs and Rust e2e
tests if not folded in, the changelog fragment and the TODO.md update.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect: `cargo fmt` and clippy on the touched crates;
the helm crate's agent-request, approval and sessions tests, the supervisor's relay tests, the CLI's tests, the UI
crate's tests, and the touched Rust e2e tests, through `python3 scripts/record-test-run.py`; the touched browser specs
on both engines; `dprint check` on changed Markdown; the changelog check; `python -B scripts/check-test-sleeps.py` when
tests change. No real Tensorlake sandbox is authorized for this plan: cover Tensorlake through the provider-neutral path
and the fakes, and Podman with real local containers.

## Fallbacks

These departures need the scope reassessment: any per-provider rule in the agent-request path; letting an agent inside
an environment create a new environment or reach another host; an approval path that lets an environment launch from an
ordinary host through without the card.

Block, rather than work around, in any of these cases: the origin of a request cannot be attributed to an environment
host reliably; an Opus consult rejects a change you still find necessary, or Opus 5.5 cannot be reached for one.

Log these as DECISIONs: the per-verb table as built; the listing filter; the flag names; the card layout; what the
browser specs could not cover; every review finding or consult verdict you decided not to follow.

## Changing the core abstractions

The maintainer requires an Opus consult before any significant change to the core abstractions in
`podman-environments.md` ("Core abstractions", as landed). Significant means changing what is a trait method, an
exhaustive `ProviderKind` method or a route; adding or removing a concept; moving per-provider decisions out of the
provider implementations; changing the storage discriminator or the landed multi-session ownership rule beyond what the
lifetime decision above asks for; or giving a provider its own mechanism. Renaming or splitting a method for testability
is not.

For a significant change: write the proposal (what changes, why the landed design does not work for it, the evidence,
and the smallest alternative you ruled out) to a file in the scratch directory, and run a fresh-context review on **Opus
5.5 at high effort**, launched natively or through `$scode-harness-shellout`, separate from the per-PR code review, with
this charter:

> Judge whether this proposed change to the core abstractions is necessary and whether it keeps the general concept (a
> provider of disposable environments, with Tensorlake and Podman as implementations) encoded in code so that a third
> provider is a new implementation with compile errors at every decision it must make. Identify whether the existing
> design could handle the case with a smaller change, whether the proposal scatters provider decisions or adds a second
> mechanism, and what the smallest coherent change is. Write findings to the named file; change no product files or VCS
> state.

Follow its verdict or log a DECISION explaining why not; a change it rejects that you still find necessary, and that no
smaller change covers, is a block. If Opus 5.5 cannot be reached, block rather than proceed.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-environment-agent-operations-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that root
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
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type,
`$scode-harness-shellout` supplies the launch mechanics.

The review gate and the Opus consult keep the models and efforts this file demands; the scope reassessment runs at your
own model and effort. The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the
resume check, and the cold reads of a blocked question and of the report) run as that file says: native, read-only, at
your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory, `/tmp` and Podman's
storage, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS).
It writes each sample to a private heartbeat/status file in the scratch directory, and it emits a notification when a
watched filesystem drops under 10% or under 5 GB free or available memory drops under 10% (whichever comes first), when
the number keeps falling, when the condition recovers, and when the monitor itself fails. In Claude Code a notification
is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's equivalent. A file update alone is
not a notification.

Before relying on the watchdog, verify delivery with a harmless synthetic alert, and verify failure detection by killing
a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a throwaway monitor
without killing it and confirm you are notified within two sample intervals. If any of these notifications is
unavailable or unverified, say so in the log and check the status file at least once a minute. On every path, check
heartbeat freshness before each new build, test run, sub-agent or review launch; a dead monitor, or a heartbeat stale
for two sample intervals, pauses new launches until monitoring is restored.

An alert is an instruction to act: stop launching work, remove jj workspaces, build output, scratch, containers and
images you own that are no longer needed, wait for or stop the job most likely responsible, and resume only when the
watchdog reports headroom. Record the watchdog's handle, watched paths, status path and delivery mechanism in the log,
include its state in every handoff note, and stop it when the plan closes. Do not lengthen the sampling interval to save
turns. Remove every container and image your tests created when you are done with them, and never touch containers or
images you did not create.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/environment-agent-operations/<nn>-<short-name>`.
- Avoid churn; within this run, a PR that needs correcting is restructured rather than corrected on top, including PRs
  an earlier run built.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's
  cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`. PRs stay drafts: never mark one ready and never merge; landing is the monitor's job.

### Code review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on **gpt-6.1-sol at high effort**,
launched through `$scode-harness-shellout` (or natively, if your harness serves that model natively). If it cannot be
reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions,
rules and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md`
verbatim in the prompt, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the merits and log
the DECISION. A finding that would significantly change the core abstractions goes through the Opus consult before you
act on it.

### Scope reassessment

Before implementing a substantial departure from the outline (examples under Fallbacks; not a blacklist), or when the
same component has needed repeated corrective review rounds, run a fresh-context review at your own model and effort.
Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered, including every review finding and consult verdict you decided not to follow.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Wording in this file that turns
out inaccurate (a line number, a file name, a type name) is corrected to the truth. A material scope expansion, a
weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's decision; a review finding or
a log entry is not authorization. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you and logged as a DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should
know and possible follow-ups go into the report's "things you should know" and "open questions and possible follow-ups"
sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its code review gate, every significant change to the core abstractions passed its Opus consult, and the last PR
updates TODO.md as described. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12). Never edit
`plans/` yourself.
