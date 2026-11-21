# Standardize model family names and shorten wrong-agent refusals

Written against main at a5157ee92 on 2026-11-20. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Two closely related TODO.md entries (bucket "Near term", subsection "Trivial"), merged into one plan with the
maintainer's agreement, verbatim:

> **Make the model family names consistent.** Every model in the launcher's model list belongs to a family (Sol, Fable,
> GLM and so on), and the families are named in three different styles: Sol, Astra, Luna and Terra by a plain word
> (`sol`), Claude and Muse by a full model ID (`claude-fable-5`, `muse-spark-1.3-contributor`), and GLM and Grok with a
> version in the name (`glm-5.3`, `grok-4.5`). A name with a version stops fitting once a successor ships, and the words
> you can type in the launcher to get a family's newest model (`fable`, `spark`, `glm`, ...) need a separate lookup
> table to map onto these names (`FAMILY_WORDS` in `crates/farhelm-proto/src/launcher.rs`). Pick one style, most likely
> the typed word itself, rename the families, and drop the lookup table if it becomes redundant. Raised by the
> model-families landing, 2026-11-19.

> **Shorter refusal for a model family word typed with the wrong agent.** With Claude chosen in the launcher, typing
> `sol` in the model field and pressing Enter says "sol is offered by Codex or Goose or Pi or OMP or OpenCode; choose
> Codex or Goose or Pi or OMP or OpenCode to use it". It is accurate but long and clumsy, no test covers it, and SPEC.md
> does not describe it. Something like "Claude has no sol model; choose an agent that offers it." would do; add a test
> and a SPEC.md line. In the same change, drop the family word that the helm sends to the app with every model: both are
> the same build and already have the word table, so that protocol field and its roughly 35 test lines add nothing; let
> the app work out the word itself. Raised by the model-family-shorthand landing, 2026-11-19.

### Where it stands (from planning, at main a5157ee92)

- `crates/farhelm-helm/src/launches.rs` (`CATALOG`, lines 106–486) names families in three styles: plain words (`neon`,
  `argon`, `hydrogen`, `flash`, `auto`, `luna`, `terra`, `sol`, `astra`), full model IDs (`claude-fable-5`,
  `muse-spark-1.3-contributor`), and versioned names (`composer-2.5`, `glm-5.3`, `glm-5.3-flash`, `grok-4.5`,
  `grok-4.6`). Both `grok-4.5` and `grok-4.6` currently have `latest: true` under separate families.
- `crates/farhelm-proto/src/launcher.rs` holds `LaunchCatalogModel.family_word: Option<String>`, the key-to-word table
  `FAMILY_WORDS`, `catalog_family_word`, and `is_catalog_family_word`. `launcher_catalog()` populates `family_word` from
  `catalog_family_word(model.family)` for `latest` rows, and `crates/farhelm-helm/src/sessions_tests.rs:570` asserts on
  `model["family_word"]` in the JSON response.
- `is_catalog_family_word` is called at three pre-catalog-load sites
  (`crates/gui/farhelm-gui/src/launcher/search.rs:537` and
  `crates/gui/farhelm-gui/src/launcher/view/model_field.rs:174,284`) so typing a family word while `/api/launch-catalog`
  is still loading shows `"Loading models…"` instead of treating the word as a custom model ID or session name
  (`SPEC_impl.md:8817-8818`).
- `crates/gui/farhelm-gui/src/launcher/search.rs` preselects `Group::Models` ahead of `Group::Harnesses` in unlabelled
  `Scope::All` (`search.rs:755-784`) and hides rows behind the pre-catalog gate (`search.rs:537`). Making `"grok"` a
  family word would break `SPEC.md:1133-1134` ("Existing search words such as `grok` keep their meaning") and
  `exact_family_words_preselect_current_models_without_reordering` (`search_tests.rs:170-171`) unless unlabelled exact
  harness words keep their harness meaning when no harness is selected.
- `crates/gui/farhelm-gui/src/launcher/model.rs` (`owned_elsewhere_message`, lines 221–228) builds
  `"{id} is offered by {labels}; choose {labels} to use it"` for both family words and exact model IDs whenever
  `ModelEnterTarget::OwnedElsewhere` fires. `OwnedElsewhere` is only constructed when `harness` is `Some(current)`.
- `SPEC.md:1067-1068` currently says: "A latest model only other harnesses offer is refused with a message naming them."

### Acceptance criteria

1. **Consistent, versionless family names.** Every model in `CATALOG` (`crates/farhelm-helm/src/launches.rs`) uses a
   plain, versionless family name:
   - `claude-fable-5` → `fable`
   - `muse-spark-1.3-contributor` → `spark`
   - `glm-5.3` → `glm`
   - `glm-5.3-flash` → `glm-flash`
   - `composer-2.5` → `composer`
   - `grok-4.5` and `grok-4.6` → `grok`, with `grok-4.6` marked `latest: true` and `grok-4.5` marked `latest: false` (so
     `grok-4.5` moves under **Older models** while remaining selectable by exact ID).
   - Existing plain names (`neon`, `argon`, `hydrogen`, `flash`, `auto`, `luna`, `terra`, `sol`, `astra`) stay as they
     are.
2. **Every family name is a launcher shorthand, and `family_word` leaves the wire.**
   - Remove `family_word` from `LaunchCatalogModel`, remove `catalog_family_word`, and remove the key-to-word
     translation table `FAMILY_WORDS`.
   - `LaunchCatalogModel::matches_family_word(&self, query: &str)` matches directly against `self.family`
     (`self.latest && !self.family.is_empty() && self.family.eq_ignore_ascii_case(query)`), so every family name in the
     catalog works as a shorthand word in the launcher.
   - Preserve the pre-catalog-load `"Loading models…"` wait (`SPEC_impl.md:8817-8818`) via a flat `CATALOG_FAMILIES`
     slice in `crates/farhelm-proto/src/launcher.rs` backing `is_catalog_family_word`, pinned by a test in
     `crates/farhelm-helm/src/launches.rs` asserting exact set equality with the distinct `model.family` values in
     `CATALOG`.
   - Remove `family_word` from test literals across `farhelm-proto`, `farhelm-helm` (including `sessions_tests.rs`), and
     `farhelm-gui`.
3. **Exact harness words keep their meaning in unlabelled search.** Typing `grok` in unlabelled launcher search
   (`Scope::All`) with no harness chosen still selects the Grok harness (both before and after the catalog loads),
   preserving `SPEC.md:1133-1134` and `search_tests.rs:170-171`. Under `model:grok`, when a harness is already chosen,
   or in the launcher's Model field, `grok` resolves to the `grok` model family as expected.
4. **Shorter wrong-agent refusal wording.** When a chosen agent does not offer a typed family word or latest model ID
   that other agents offer:
   - When multiple other agents offer it: `"<ChosenAgent> has no <query> model; choose an agent that offers it."` (for
     example, with Claude chosen and `sol` typed: `"Claude has no sol model; choose an agent that offers it."`).
   - When exactly one other agent offers it: `"<ChosenAgent> has no <query> model; choose <Owner> to use it."` (for
     example, with Codex chosen and `sonnet` or `fable` typed: `"Codex has no fable model; choose Claude to use it."`).
5. **Specs and tests follow.**
   - Update `SPEC.md` (both the model-field wrong-agent refusal line around line 1067–1068 and the family-word shorthand
     paragraph around line 1074–1078) and `SPEC_impl.md` (around line 8807–8820, describing versionless family names and
     the test-pinned pre-catalog family list without `family_word` on the wire).
   - Add regression tests (per `.agents/test-authoring.md`) covering: catalog family parity with `CATALOG_FAMILIES`,
     `grok-4.6` latest / `grok-4.5` older, `grok` keeping its harness meaning in unlabelled search while resolving as a
     model family under `model:grok` and the Model field, and both single-owner and multi-owner wrong-agent refusals for
     family words and exact IDs in `model_tests.rs` and `view_tests.rs`.
6. **Cleanup.** Remove both covered TODO.md entries in the PR and leave the required changelog fragment.

## Decisions already made

Settled with the maintainer on the 2026-11-20 planning page:

- **Merge:** Merge "Make the model family names consistent" and "Shorter refusal for a model family word typed with the
  wrong agent" into one plan (`model-family-cleanup`).
- **Family names and shorthand words:** Use clean versionless family names everywhere, fold `grok-4.5` as an older
  member of `grok` (`grok-4.6` latest), and drop the separate shorthand lookup table so every family name works as a
  shorthand word (including `neon`, `argon`, `hydrogen`, `flash`, `composer`, `grok`, `glm-flash`, `auto`).
- **Refusal wording:** `"<ChosenAgent> has no <word> model; choose an agent that offers it."` when multiple agents offer
  it, and `"<ChosenAgent> has no <word> model; choose <Owner> to use it."` when only one agent offers it, for both
  family words and exact model IDs.
- **Review gate:** A single fresh-context `gpt-6.2-sol` agent at `high` effort with the general review charter.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** The plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** The two TODO entries and the decisions above.

**Binding repository constraints:**

- `SPEC.md` ("Launcher": model field, family shorthand, search precedence around lines 1065–1135) and `SPEC_impl.md`
  (catalog family words around lines 5212 and 8807–8820).
- `.agents/test-authoring.md` for every test added or changed, and `scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and "Code rules" (exhaustive
  per-harness functions).

**Planner proposals** (verified against the code and planning review):

- **Keep `CATALOG_FAMILIES` only for pre-catalog waiting.** Because `farhelm-gui` depends on `farhelm-proto` and not
  `farhelm-helm`, keep a flat `CATALOG_FAMILIES: &[&str]` slice in `crates/farhelm-proto/src/launcher.rs` for
  `is_catalog_family_word` (the three pre-catalog-load `"Loading models…"` checks) and assert exact equality with the
  set of `model.family` values in `CATALOG` in `crates/farhelm-helm/src/launches.rs`. Do not move `CATALOG` into
  `farhelm-proto`.
- **Preserve exact harness words in `search.rs`.** In `crates/gui/farhelm-gui/src/launcher/search.rs`, when
  `scope == Scope::All && harness.is_none()` and the query matches an exact harness word (derived from
  `HARNESS_SEARCH_ORDER` / harness search labels, not a hardcoded `"grok"` special case), skip the pre-catalog
  family-word wait (`search.rs:537`) and do not let a model-family match preselect `Group::Models` ahead of
  `Group::Harnesses` (`search.rs:720-784`).
- **Carry `chosen: LaunchHarness` on `ModelEnterTarget::OwnedElsewhere`.** Because `OwnedElsewhere` is only ever
  constructed when `harness` is `Some(current)`, storing `chosen: LaunchHarness` on the variant avoids an `Option`
  unwrap at `model_field.rs:201-202`.

## Outline

One PR:

1. `plan/model-family-cleanup/01-model-families-and-refusal` (`feat:`): rename `CATALOG` families to versionless words
   (folding `grok-4.5` under `grok`), drop `LaunchCatalogModel.family_word` and `FAMILY_WORDS`, add test-pinned
   `CATALOG_FAMILIES`, keep unlabelled exact harness words winning in `search.rs`, update `owned_elsewhere_message` and
   its callers, update `SPEC.md` and `SPEC_impl.md`, update and add tests across `farhelm-proto`, `farhelm-helm`, and
   `farhelm-gui`, add a changelog fragment, and remove both TODO.md entries.

## Fallbacks

- If any existing test relies on `"auto"` or `"flash"` being treated as a custom model ID on another harness (rather
  than an `OwnedElsewhere` refusal), inspect the test's intent: if it was testing arbitrary custom IDs with an unlucky
  sample string, change the test's sample ID to a non-family string and log it; if it pins a documented SPEC rule, check
  SPEC.md and block only if SPEC and the maintainer's decision genuinely conflict.
- Decisions to log: how `search.rs` detects an exact harness word, any test fixture sample IDs updated for new family
  words, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-model-family-cleanup-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-model-family-cleanup-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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
  `plan/model-family-cleanup/01-model-families-and-refusal`.
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
- The last PR of the stack removes both of this plan's TODO.md entries.

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
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing both of this plan's
TODO.md entries. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then
close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the
queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
