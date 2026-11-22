# Container hosts follow the machine and account rules ordinary hosts do

Written against main at d6157939f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Apply host identity rules to Podman places.** If an SSH alias starts reaching another machine or account, a Delete
> can see no container there and forget a live one on the original host. Two aliases for the same machine and account
> can also both appear usable for new containers. Use one stable machine/account identity for both outcomes. Record it
> for the execution endpoint before sending a new container create, and dispatch only to that verified endpoint. Verify
> the identity on the connection that reports absence before accepting that as proof of deletion. Refuse new containers
> on a second place for the same machine and account while preserving safe cleanup for existing bindings. Honor the
> ordinary supervisor's identity refusal when it is available and keep support for places without a supervisor. The
> relevant code is in `environment/podman.rs`, `environment/podman_probe.rs` and `store/podman.rs` in the helm. For an
> older binding whose create may have been sent, finding the owned container can establish its identity. If the
> container is absent, the helm cannot distinguish a removed container from a changed route. Decide how users can clear
> those rows without assuming the current endpoint is the original one. Cover both the retargeted alias and the
> duplicate place in tests and update SPEC. The Podman triage plan deferred the pair together because the older-row
> behavior is unsettled.

The underlying decision is the maintainer's "Apply the ordinary identity rules" from the 2026-11-21 triage, recorded in
`TRIAGE_OUTCOMES_ARCHIVE.md` under `retargeted-ssh-alias-can-forget-live-container.md` and
`podman-place-probe-bypasses-host-identity-refusal.md` (deferred to TODO.md by PR #4504). Its completion criteria:
record the machine and account identity when a container is bound, refuse to accept "absent" as proof of deletion under
a different identity, refuse twin container hosts that reach the same machine and account, and update SPEC.

In product terms: an ordinary ssh host already refuses to connect when its destination reaches a different Farhelm
install (the user chooses Adopt or fixes the destination), and refuses a second entry that reaches the same install as a
duplicate. A host used for Podman containers gets neither. So when the user repoints a container host's ssh alias
outside Farhelm, Delete asks the new machine, finds nothing, reports success and forgets a container that is still
running on the old machine; and two entries reaching one machine and account are both offered for new containers.

### Where it stands (from planning research at main d6157939f; line numbers approximate)

- **Ordinary hosts' identity** is a UUID the supervisor mints on first use for one Farhelm install in one account
  (`crates/farhelm-supervisor/src/store.rs`, `ensure_host_identity`, about 6233), reported in its hello. The helm
  records it in `hosts.host_identity` with a unique index (`crates/farhelm-helm/src/store.rs`, about 68 and 2962-2969),
  settled inside `record_first_contact` and `adopt_identity`. The manager's refusals: `IdentityMismatch`,
  `IdentityUnverified` and `Duplicate { twin, identity }` (`manager.rs`, about 496-568). A duplicate row never records
  the identity. The desktop app's text: `crates/gui/farhelm-gui/src/hosts/identity/phase.rs`, about 551. That identity
  cannot serve container hosts: places need no supervisor, and a reinstall changes the UUID while Podman storage stays.
  Nothing reads `/etc/machine-id` or a uid as an identity today.
- **A Podman place** is any Local or ordinary SSH host entry (`Place::PodmanHost(i64)`,
  `crates/farhelm-proto/src/environment.rs`, about 27; `can_host_environments`, `store.rs`, about 886). Places are Linux
  only (`podman_probe.rs`, about 656), and systemd is required. `place()` (`environment/podman.rs`, about 185) only
  checks that the route still equals the cached probe's route. The probe cache is keyed by host id and route
  (`podman_probe.rs`, about 284-297): `ready()` (about 368) wants a result under 5 minutes old, `runtime()` (about 382)
  accepts an older success, `probe_inner` (about 645) records OS, architecture, Podman path, `podman info`, proxies and
  a scope test, but no identity. Every command is its own ssh run of `sh -c CLEAN_PLACE_EXEC …` (`podman_probe.rs`,
  about 162; `access.rs`, about 18) over a shared ControlMaster.
- **Per container,** `PodmanBinding` (`store/podman.rs`, about 12) stores name, recipe, `place_host`, an optional
  `container_id`, the login and `pending_create_at`; no identity. A place cannot be retargeted inside Farhelm while rows
  or recipe artifacts reference it (`store/podman.rs`, `refuse_referenced_place`, about 214), so the bug needs an alias
  changed outside Farhelm.
- **Create and absence.** `create` (`podman.rs`, about 1181) writes `set_podman_prepared_create` (about 1249), then
  `run` (about 478); a recovery path re-sends `run` when the name is absent after the settle window (about 1235).
  Absence is trusted in `destroy` (about 1440, 1475), `remove_container` (about 451, 466), `status` (about 1397,
  "deleted outside Farhelm") and `clear_refused_podman_create` (about 1281). Every absence check goes through
  `Podman::exists`, and every create through `scoped_output`.
- **Fakes.** `PlaceCommands` has several test implementations (`environment/podman/tests.rs`, about 60;
  `environment/podman/failure_tests.rs`, about 348; `podman_probe.rs` tests; `access.rs`, about 461;
  `ui_channel_tests.rs`). A new trait method with a default implementation avoids touching all of them.

### Acceptance criteria

1. **The identity.** A container host's identity is its machine id (`/etc/machine-id`) plus the account (its numeric
   user id), read by the probe over the same connection it already uses. It works with or without Farhelm installed on
   the host.
2. **The place claim.** A successful probe claims the identity for its host entry in the helm's database, under a unique
   constraint, settled in one transaction, following the existing `hosts_identity_claim` pattern:
   - another entry already holds it: this entry is a twin;
   - this entry recorded a different identity and has containers or recipe images or helpers on it (the condition
     `refuse_referenced_place` already uses; share that helper): the place has changed;
   - this entry recorded a different identity and nothing references it: it re-claims, as a retarget inside Farhelm
     would allow. **Tie rule:** when one of two twins holds the ordinary supervisor identity claim and the other holds
     the place claim but has nothing on it, the entry holding the ordinary claim takes the place claim over, so the
     ordinary claim and the place claim never pick different winners and leave neither usable. Otherwise the first to
     claim wins.
3. **Twins and changed places are refused for new work.** New containers, builds and prepared images refuse a twin or
   changed place; existing containers on a twin whose identity matches can still be checked, stopped and deleted. An
   entry whose ordinary host connection is refused as `Duplicate`, `IdentityMismatch` or `IdentityUnverified` is also
   refused for new containers. A place without a supervisor relies on the place claim alone.
4. **New containers record where they run.** `set_podman_prepared_create` records the place identity on the binding in
   the same guarded update, before the create is sent, and the create runs only after the identity is verified on that
   same remote shell.
5. **Absence counts only under the recorded identity.** For a container with a recorded identity, every absence check
   verifies the identity in the same remote shell that runs the Podman command (one ssh run answers both "who is this"
   and "is it there"), through a new `PlaceCommands` method with a default, applied in `Podman::exists` and the create
   path. On a mismatch, Delete keeps the container and its session and shows the reason with Retry; status does not
   report "deleted outside Farhelm"; the create recovery does not re-send.
6. **Older containers are left as they are** (Decisions): a binding with no recorded identity behaves exactly as today,
   including trusting absence on Delete. No adoption of an identity for older rows, no new confirmation, no Forget
   action. Recipe images and helpers already on a place adopt the first claim after upgrade.
7. **What users see.** The refusals use the existing reason surfaces (the launcher, Settings → Containers, the session's
   Delete failure with Retry) and plain, jargon-free wording that says the likely cause (Decisions). Starting points,
   which you may refine without losing the hints:
   - twin: "This host reaches the same machine and account as <other host>, so new containers go through <other host>.
     If these are two different machines, one was probably copied from the other (a cloned virtual machine, for example)
     and kept the same machine ID; give the copy a new machine ID and press Refresh. Otherwise remove one of the two
     entries or change its destination."
   - changed place on Delete: "This container host now reaches a different machine or account than the one this
     container was created on, so Farhelm cannot check or delete the container through it. Its SSH destination may have
     been changed outside Farhelm; restore it, then press Retry."
   - changed place for new containers: the same cause and remedy, worded for a host rather than one container.
8. **Tests,** without sleeps (`.agents/test-authoring.md`): a retargeted alias (Delete keeps the container and shows the
   reason; a status check does not report it deleted; a create recovery does not re-send); a twin place refused for new
   containers while an existing container on it can still be deleted; the tie rule; an ordinary-connection refusal
   honored; an older binding with no identity behaving as today.
9. **Spec.** SPEC.md, about 322-327 (identity rules extended to container hosts), about 5260-5263 (probe and unusable
   reasons, twins), about 5292-5296 (Delete's confirmed absence is under the recorded identity; older containers as
   today), about 5321-5323 (the retarget refusal), and about 5374-5377 (an alias retargeted outside Farhelm is not
   excused by the "deliberate setup outside Farhelm" line); SPEC_impl.md, about 6045-6052 (binding facts) and about
   6163-6171 (probe and claim).
10. The stack carries a changelog fragment and its last PR removes the TODO entry.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Older containers: the absolute minimum.** The maintainer's words: "whatever adds the absolute minimum amount of
  code. we don't care about this. no users are using this yet includint myself. just make it work going forward." So
  containers created before this change behave exactly as today; only new containers get the identity check. This
  settles the TODO's open "decide how users can clear those rows": they clear as today.
- **Identity: machine id plus account.** Chosen over a marker file Farhelm would write into each container host's
  account, after the maintainer was told that a VM copied from an already-booted machine keeps its original's machine id
  and would be refused as the same machine until the id is regenerated. With the condition, in the maintainer's words:
  "make it so that the error message the user sees has clear understandable to the user hints about the likely cause
  here, in user facing language without internal jargon" (acceptance criterion 7).
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the triage decision "Apply the ordinary identity rules" and
its completion criteria, and the decisions above (which narrow the TODO's older-row handling to "as today").

**Binding repository constraints:**

- SPEC.md's host identity rules (about 322-327) and the container passages named in criterion 9.
- Root `AGENTS.md`, "Code rules": every disposable-environment provider decision belongs in its implementation or an
  exhaustive method, per the map in `crates/farhelm-helm/src/environment/mod.rs`; read it before changing provider
  behavior. Shared code compares no individual providers. UI wording belongs in one exhaustive UI module.
- A database migration follows the helm's existing migration ladder and its historical-schema fixtures.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and its "Instructions by path" for every path you touch.

**Planner proposals** (from research and an independent planning review):

- Two separate identities, the ordinary supervisor identity and the place identity, rather than one: places may have no
  supervisor, and a reinstall changes the supervisor's identity while the containers stay. The tie rule keeps them from
  disagreeing.
- Both a place claim and a per-container identity: the place claim gives the durable twin refusal and covers recipe
  images and helpers; the per-container identity satisfies "record before send" and serves containers on a twin row. For
  a new container on a non-twin place the two always match; compare only one per operation.
- Limit the same-shell check to `Podman::exists` and the create path; every absence decision and every create already
  pass through them, which also covers recipe image and helper cleanup.
- Honoring the ordinary connection's refusal mainly keeps the container view consistent with the Hosts list; a handle
  from the place code to the manager set after construction (like `publish_to`, `podman_probe.rs`, about 334), or a
  check where the manager already lists places, are both acceptable. Decide and log.
- An SSH entry pointing at this computer becomes a twin of the Local container host; that is intended.
- Fragment, `kind: fixed`, along the lines of: "Deleting a container session no longer forgets a container that is still
  running when the container host's SSH destination now reaches a different machine or account, and two container hosts
  that reach the same machine and account are no longer both offered for new containers."

## Outline

1. `plan/podman-place-host-identity/01-place-identity-claim`: the probe reads the identity; the migration and place
   claim with the tie rule; twin and changed-place refusal for new work; honoring the ordinary connection's refusal; the
   twin and changed-host wording; tests for the twin, the tie rule and the honored refusal; the spec text for twins.
   `fix:` (for example `fix: refuse a second container host for the same machine and account`), with a fragment.
2. `plan/podman-place-host-identity/02-container-identity-check`: the per-container identity recorded before create; the
   same-shell verification in `Podman::exists` and the create path; Delete, status and create recovery on a mismatch;
   older bindings unchanged; tests for the retargeted alias and the older binding; the remaining spec text; editing PR
   1's fragment to cover both; the TODO entry's removal. `fix:` (for example
   `fix: never treat a container as deleted on a host that moved`).

## Fallbacks

- If one PR turns out clearer than two (the claim and the check share most of their plumbing), merge them and log it.
- If the ordinary-connection refusal cannot be reached from the place code without a broad new dependency, keep the
  refusal to the place claim alone, log it as a DECISION, and name the gap in the report.
- If reading the account as a numeric user id is awkward on some supported host, use the account name instead and log
  it.
- A reviewer finding that asks for older-row adoption, a Forget action, a confirmation flow or a marker file is declined
  by decision; log it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-podman-place-host-identity-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-podman-place-host-identity-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes; when the machine has rootless Podman, the
real Podman lifecycle check `.agents/validation.md` lists is worth one run at the end. Apply `.agents/test-authoring.md`
to every test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares
it when Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
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
- The last PR of the stack removes the TODO.md entry this plan covers ("Apply host identity rules to Podman places"), as
  root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides. A helm schema version that another plan
  also took is renumbered at rebase (`.agents/careful-rebase.md`).
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
