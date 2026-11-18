# Known models, with a latest model per family

Written against main at 04631d86 on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan changes what they say about the launcher's models.

## The goal

The TODO.md entry (bucket "Plan decisions to revisit"), verbatim:

> **Revisit letting saved templates with a retired Sol, Astra or Zen model ID still pick their agent.** Decided on the
> maintainer's behalf on 2026-11-18 when launcher-gpt-6-2-models blocked: once the launcher stops suggesting the old Sol
> and Astra IDs (bare, and the two OpenCode Zen ones), a saved template that names one of them but no agent type would
> be refused from an empty launcher, because the agent was inferred from the suggestion list. The plan was told to keep
> inferring Codex or OpenCode for those four IDs in that case only, keeping the saved ID as a custom model, and to
> record this in SPEC.md as a narrow exception to templates applying exactly like hand edits. The alternative was making
> affected users add an agent type to such templates themselves.

Revisiting it, the maintainer kept the outcome (old model names keep working) and asked for the general mechanism
instead of the special case. In their words (2026-11-18):

> I think it's fine to accept old versions of various models that we know are real models. The main thing is that if
> somebody just asks for "sol", we should default to the latest version. […] i think accumulating known present-day
> models associated/known ot be associated with agents is fine, until they become obviously obsolete and are not
> relevant anymore. so: fundamentally, having a larger total set we _accept and recognize_ than what we would ever
> choose by default or recommend etc seems fine, and the tensino here I think is stemming from us not having this today.
> so can we fix taht by having a notion of a known set of models with a clear latest default for a given family (so e.g.
> right now sol-6.2 is the latest in the sol family) and not force immediate aging out of the previous model, leaving
> that to clearly old obsolete models in the future.

Today the helm's one model list (`CATALOG` in `crates/farhelm-helm/src/launches.rs`) plays two roles: what the launcher
suggests, and what Farhelm recognizes (which agent runs a model and which effort levels it has). Dropping a model from
the suggestions stopped Farhelm recognizing it, which is why 0.41.0 needed a four-name special case for saved templates.
After this plan the list is a set of _known_ models, each in a _family_, with one _latest_ model per family.

### Acceptance criteria

1. **Known models with families.** Each known model records its agent (harness), its effort levels, and its family, and
   each family marks exactly one model as its latest _per agent_ (the latest Sol is a separate entry for each agent that
   runs it). Every model the launcher suggests today is the latest of its own family, so nothing suggested today moves
   under the "Older models" heading: Sol and Astra are families with an older member, while models like Luna, Terra,
   each Grok and GLM version, and Cursor's `auto` and `composer-2.5` are each their own family until a newer version
   replaces them.
2. **Older known models from the start:** the ones the launcher stopped suggesting in 0.41.0, for every harness that
   offered them: `gpt-6.1-sol` (Codex), `gpt-6-astra` (Codex), `opencode/gpt-6.1-sol` and `opencode/gpt-6-astra`
   (OpenCode), and `openai/gpt-6.1-sol` and `openai/gpt-6-astra` (Goose, Pi and OMP).
   `git show 03cd2ad17^:crates/farhelm-helm/src/launches.rs` has the entries as they were. Effort levels: each gets the
   list it would accept in 0.41.0, where it was compiled as a custom model with its harness's full list. That includes
   Codex `gpt-6-astra` with the full Codex list, not High only as before 0.41.0, because a 0.41.0 template with Codex,
   that model and Low or Medium effort must keep working (criterion 5).
3. **Suggestions.** Wherever the launcher suggests models (the model dropdown, its search as you type, the dropdown with
   no agent chosen, and the template editor's model suggestions), the latest models come first, as today, and the older
   known models follow under an "Older models" heading, for the chosen agent when one is chosen. The maintainer chose to
   show them (2026-11-18). The heading uses the headings the shared suggestion-list code already draws (as the folder
   list's "Recent" does), not a new component. A past launch of an older model still shows as a past-launch row, as
   recent launches do today.
4. **Recognized everywhere.** Every known model, latest or older, is recognized wherever a model name is accepted: typed
   in the launcher, in a saved template, in a recent launch, in an agent's launch request, and on the command line.
   Recognized means what it means for a suggested model today: when no agent is chosen and exactly one agent runs that
   model, Farhelm picks that agent, and the model's effort levels apply. A name more than one agent runs (the `openai/`
   ones) picks no agent, as today.
5. **The special case goes.** The four-name exception from 0.41.0 (the `apply_template` arm in
   `crates/farhelm-proto/src/launcher.rs`, its tests, the GUI's `choose_history_model` path in
   `crates/gui/farhelm-gui/src/templates/apply.rs`, the comments in `launches.rs` and `agent_launch.rs` telling future
   model changes to add their old names, and the SPEC.md and SPEC_impl.md paragraphs naming the four IDs as a narrow
   exception to templates applying exactly like hand edits) is replaced by criterion 4, which makes a saved template and
   the same hand edit behave the same again. Every template that worked in 0.41.0 keeps working.
6. **Retiring a model is a deliberate edit.** Nothing ages out by itself. SPEC.md says the known set grows as models are
   replaced and shrinks only when someone removes a clearly obsolete model; SPEC_impl.md says where the list lives and
   how a model change adds a new latest and keeps the previous one as older.
7. **The family's latest is available to callers,** so the separate Near term entry "typing sol picks the latest Sol
   model" (added in this plan's planning PR) can resolve a family name without another catalog change. This plan does
   not implement that shorthand.
8. **Tests:** the catalog's own tests (one latest per family per agent; no id listed twice for one harness; every older
   model's family has a latest for its agent), the inference tests in `crates/farhelm-proto/src/launcher.rs` rewritten
   from the special case to the general rule (a saved template naming an older model with no agent applies with the
   inferred agent; so does the same name typed by hand), the GUI's template-application test, and the helm route test
   that serves the catalog (`composer_catalog_and_history_routes_serve_helm_owned_choices` in
   `crates/farhelm-helm/src/sessions_tests.rs`).
9. A changelog fragment per root `AGENTS.md`, "Changelog fragments", for each user-visible PR, and the last PR removes
   this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page:

- **Keep accepting the old names, through a general known-models set with a latest per family,** instead of the
  four-name special case (the maintainer's words above). The planner described the design (criteria 1 to 7) on the page;
  the maintainer answered every follow-up question built on it without objecting to the design itself.
- **Older known models are shown,** below the latest ones under "Older models". Rejected: hiding them unless the user's
  own recent launches used them.
- **The initial older set is what 0.41.0 dropped** (criterion 2), for every harness.
- **"Sol" alone picking the latest Sol model is a separate fix**, recorded as its own Near term TODO entry in the
  planning PR.
- **No release note** (see Release note); each user-visible PR still leaves its changelog fragment.
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the quoted answer, and the Decisions above.

**Binding repository constraints:** SPEC.md's launcher model paragraphs ("The helm owns the released model catalog", the
template application rule, the custom-model rule that "a custom model needs an explicit harness"), SPEC_impl.md's
catalog passages; root `AGENTS.md`, "Code rules" (per-harness behavior lives where
`crates/farhelm-supervisor/src/agent_kind/mod.rs`'s map says; the catalog is that place for models), "Changelog
fragments", "Conventional Commits", "Tests and validation"; `.agents/test-authoring.md`.

**Planner proposals** (from reading the code at 04631d86; follow the code where it differs, logged as a DECISION):

- Extend `CatalogModel` with a family and a latest flag rather than keeping a second list of retired names. `compile` in
  `launches.rs` already applies per-model checks only to ids in `CATALOG`, so adding the older entries there gives them
  their effort checks.
- The GUI learns models from the helm's catalog route (`GET /api/launch-catalog`), and the desktop app always talks to
  the helm built with it (SPEC_impl.md, "Version and skew"), so changing the route's shape costs only in-tree consumers.
  Add the family and the latest marker as fields on the existing row, defaulting to "latest" so the JSON test stubs stay
  valid (about 30 test struct literals still gain the field). A separate list would need about seven lookups changed.
- The template fold in `crates/farhelm-proto/src/launcher.rs` already reads the whole served catalog, so once the older
  models are served, criterion 4 for templates is the deletion of the four-name exception.

## Outline

1. `plan/model-families/01-known-model-families`: the catalog's families, latest marks and older entries (criteria 1, 2,
   6, 7), the route shape, and recognition over the whole known set with the special case removed (criteria 4, 5),
   SPEC.md and SPEC_impl.md, tests. In this PR the GUI keeps older models out of every suggestion list (recognized
   only), so main never shows them mixed in with the latest ones. `feat:`, with a fragment.
2. `plan/model-families/02-older-models-in-launcher`: the "Older models" heading on every suggestion surface
   (criterion 3) and its tests. `feat:`, with a fragment. Removes this plan's TODO.md entry.

If PR 1 grows past a comfortable review size, split the inference and special-case removal into its own PR after the
catalog change; never leave the special case and the general rule both in place on main.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the changed crates, the proto launcher tests, the helm's launch catalog and session-route unit tests, the GUI crate's
launcher and templates unit tests (all through `python3 scripts/record-test-run.py`), the GPUI goldens if the model
dropdown's drawing changes, and `python3 releasing/check-changelog.py format`.

## Fallbacks

- If serving older models to the GUI needs a protocol version change between helm and supervisor (not only the GUI's
  route), that is a departure: run the scope reassessment.
- If showing "Older models" needs a new dropdown component rather than a heading in the existing suggestion list, that
  is a departure: run the scope reassessment.

Decisions to log: the catalog's shape, the route shape, how inference changed, every test changed and why, and every
review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-model-families-log.md` in the parent directory of the checkout you run in, derived as that section says,
and the watchdog's status file is `farhelm-plan-model-families-watchdog.json` beside it. Resolve both to absolute paths
before you start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests on the machine you run on, as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`. The maintainer chose this on 2026-11-18; do not use the boat sandboxes
(`.agents/boat.md`) for this plan.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (one of its remotes
points at that GitHub repository; if not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill
with the log file's absolute path. If the log exists, read it and resume where the previous session left off,
cross-checking it against reality (the jj graph, bookmarks and open PRs) rather than starting over. If it does not
exist, this is a fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the
resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

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
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/model-families/<nn>-<short-name>`, as the
  outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last code PR of the stack removes this plan's TODO.md entry.

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

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
meeting the acceptance criteria above, each having passed its review gate, and the last code PR removing this plan's
TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
