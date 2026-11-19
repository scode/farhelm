# Typing a model family picks its latest model

Written against main at d480779c on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `model-families.md` has landed (its `INDEX.md` line says so). That plan gives every known model a
family and marks each family's latest model per agent, and it says outright that it leaves this shorthand to this plan.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Typing a model family picks its latest model.** Typing just "sol" (or another family name) in the launcher's model
> field should choose that family's latest model, the way "yolo" completes to a permission; today it does not complete
> to a model at all. Asked for by the maintainer on 2026-11-18 while planning `model-families`, which records the latest
> model of each family that this needs.

### Acceptance criteria

1. In the session launcher's model field, typing exactly a family word (ASCII case-insensitive) highlights that family's
   latest model for the chosen harness, so Enter takes it. The rows and their order stay as they are (the latest models
   already sit above "Older models"). This is the field's one exception to its rule that a typed filter never becomes
   the highlighted row on its own (SPEC_impl.md), and it applies only to an exact family word.
2. In the launcher's search box, an exact family word preselects that family's latest model row on purpose, the way
   "yolo" preselects its permission, rather than landing on whatever row the catalog lists first.
3. The draft stores the concrete model id, never the family word, so templates, recent launches and Restart are
   unchanged.
4. A family word is one short word per family the catalog has that has a natural short name (for example sol, astra,
   luna, terra, fable), and each word names exactly one family. "glm" names the GLM family whose latest is the full
   model (`glm-5.3` at planning time), not the Flash family; Flash stays reachable by its model text. Families with no
   natural short word (each Grok version, Cursor's `auto` and `composer-2.5`) get none. Use the family as
   `model-families` records it if that is already a typeable word; otherwise add a family word to the catalog row,
   decided by what that plan landed. A word that is also a harness name is not a family word: "grok" stays the Grok
   harness, and the same goes for any other harness word (Muse's, say). Anything other than an exact family word (a
   prefix like "so", "grok-", "grok-4.7", a full id) is model text, filtered and matched exactly as today.
5. With no harness chosen, an exact family word that several harnesses offer asks for a harness in the model field, as
   an id several harnesses offer does today. A family word only one harness offers picks that harness and its latest
   model, as a known id with one owner picks its harness today (SPEC.md). In the search box, the first matching
   harness's latest row is preselected, with the other rows listed as today.
6. Templates (whose model field is free text) and the command line are out of scope: "sol" there stays as typed.
7. SPEC.md's typed-model rule and its search-word paragraph (around its lines 1043 to 1050 and 1072 to 1076 at planning
   time) and SPEC_impl.md's model-field rule (around its line 5029) state the family-word rule.
8. Tests: an exact family word highlights the latest for the chosen harness and Enter picks it; mixed case resolves;
   "so" does not resolve; "grok" stays the harness and "grok-4.7" stays model text; no harness asks for one; the search
   box preselects the latest even when an older model of the family is listed first; the draft holds the concrete id.
9. A changelog fragment per root `AGENTS.md`, "Changelog fragments", and the last code PR removes this plan's TODO.md
   entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page, which showed a mockup of "sol" typed with the latest Sol
highlighted:

- **One short word per family the catalog has.** Rejected: only families with an older model.
- **"grok" stays the harness.** The maintainer added, verbatim: "but if you type grok- or grok-4.7 then it's the model".
- **No harness chosen asks for a harness.** Rejected: picking Codex's model.
- **Stated on the page and not objected to:** visible highlighting so Enter takes it; the concrete id is stored; exact
  family words only; letter case does not matter; templates and the command line are out of scope.
- **Release note:** none needed (see the section at the end).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending, after `model-families.md`.**

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the Decisions above.

**Binding repository constraints:** SPEC.md's typed-model and search-word rules; SPEC_impl.md's model-field Enter rule;
`model-families.md` as landed on main (its catalog shape wins over anything this file assumes); root `AGENTS.md`, "Code
rules" (harness-specific behavior goes where `crates/farhelm-supervisor/src/agent_kind/mod.rs`'s map says; this plan
should need none), "Changelog fragments", "Conventional Commits", "Tests and validation"; `.agents/test-authoring.md`.

**Planner proposals** (from reading the code at planning time, before `model-families` landed; re-read the launcher and
catalog on main before starting, follow the code where it differs, and log it as a DECISION):

- "yolo" lives in the search box: `SearchPermission::word` (`crates/gui/farhelm-gui/src/launcher/search.rs`), and
  `default_index` preselects the row whose `exact_word` equals the query. Extend `exact_word`/`default_index` so an
  exact family word matches the family's latest row.
- The model field's `model_options` filters by substring, and `model_enter_target` (`launcher/model.rs`) already takes
  the highlighted row. So the typing handler (`model_typed` in `launcher/view/model_field.rs`, which clears the
  highlight today) sets the highlight to the family's latest row for an exact family word while a harness is chosen, and
  `model_enter_target` stays unchanged. Enter then applies it through the existing pick path.
- One shared predicate on a catalog row ("is this row the latest of family word W", in `crates/farhelm-proto` beside the
  catalog types, or wherever `model-families` put the family data) serves both surfaces: the model field highlights the
  first row for the chosen harness that passes it, and `default_index` preselects the first model row that passes it.
- This is GUI-only: no protocol, schema or terminal-host change, beyond a catalog field if criterion 4 needs one.

## Outline

One PR: `plan/model-family-shorthand/01-family-word-picks-latest`, `feat:`: the family word (a catalog field only if
needed), the shared predicate, the model field and search box, the SPEC.md and SPEC_impl.md edits, tests, the fragment,
and the TODO.md entry removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the touched crates, the launcher's model, search and view tests and the catalog tests through
`python3 scripts/record-test-run.py`, and `python3 releasing/check-changelog.py format`. No golden is expected to
change.

## Fallbacks

- If `model-families` landed family keys that are already typeable words matching criterion 4, use them and add nothing
  to the catalog.
- If a family word collides with another search word (a harness, effort or permission word), the existing word keeps
  winning, as with "grok", and the family gets no word; log it.
- If resolving in the model field needs the helm (a new route or a compile-time resolution), that is a departure: run
  the scope reassessment.

Decisions to log: where the family words come from, the helper's home, the full list of family words, every test changed
and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-model-family-shorthand-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-model-family-shorthand-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation".

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/model-family-shorthand/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's entry for the release, not an instruction to
  you: each PR still leaves the fragment its type calls for, describing what that PR built, and where what got built
  departs from the release note (a renamed control, a part left out, a limitation it does not mention), the fragment's
  context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step 8).
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
