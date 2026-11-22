# Explain stale choices in the New Session launcher

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Explain stale launcher choices.** In the empty command field, Enter on a recent setup that has disappeared can do
> nothing without saying why. Clicking a search result for a host that just vanished can also clear the query without
> selecting anything. In `launcher/view/search.rs` and `search/start.rs`, use the existing stale-result warning before
> dropping either choice. Keep the search text, and keep a withdrawn keyboard selection from falling through to Launch.
> A missing explicit host or recent choice in typed results should use the same warning. Preserve the history reset when
> the host installation changes so an old choice cannot match a different installation.

In product terms: whenever a user clicks or presses Enter on a choice in the New Session launcher (empty-field recent
setup or template, or a clicked/keyboard-selected host, template, recent setup, or other explicit pick in typed search
results) that disappeared while the launcher was open, show the launcher's inline stale-choice warning below the command
field before dropping the selection, keep any typed search text intact, never fall through to Launch on that Enter
press, and preserve host-installation identity checks so an old choice can never match a different installation's setup.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Inline stale warning (`crates/gui/farhelm-gui/src/launcher/view/search.rs:94, 694-702`):**
  - `STALE_ROW` currently reads `"That result belongs to an earlier host; choose a current result"` and renders via
    `self.search_box.refusal` (`launcher-search-refusal`, `Role::Status`, `palette.warn`) below the command field.
  - Per the maintainer's decision, generalize `STALE_ROW` to
    `"That choice is no longer available; choose a current result"` so it reads cleanly for vanished hosts, templates,
    recents, folders, and models alike.
  - Note: neither `SPEC.md` nor `SPEC_impl.md` quotes the old `STALE_ROW` text, and no golden scene renders `STALE_ROW`
    or `launcher-search-refusal`.
- **Empty command pane (`crates/gui/farhelm-gui/src/launcher/view/search/start.rs:107-152`):**
  - `take_start_highlight` silently returns when `start_highlight` is no longer in `start_rows(cx)`, leaving
    `start_highlight` stuck and showing no warning. Show `STALE_ROW` in `self.search_box.refusal`, drop
    `start_highlight`, and do NOT fall through to Launch on that Enter press.
  - `take_start_row` silently swallows `Err(RecentRefusal::Stale)` (line 148). Show `STALE_ROW` in
    `self.search_box.refusal` before dropping the choice.
  - `SetupSource::Recent` (`search.rs:114-117`) currently stores `Option<i64>` rather than `Install`. Reuse the existing
    `crate::launcher::draft::Install` (`draft.rs:399-412`, already imported in `launcher/view/history.rs`) as
    `SetupSource::Recent(Option<Install>, SetupKey)` (`None` for helm-wide `entry.target.is_some()` setups,
    `self.chosen_host(cx).map(Install::of)` for host-scoped setups), and do **not** clear `start_highlight` in
    `reset_host_history_highlight()` — otherwise Enter in the empty field after a disconnect or install change would see
    `start_highlight == None` and fall through to `self.press` (Launch). Instead, an install change or disconnect makes
    the bound `Install` fail to match any live row, so Enter reaches `take_start_highlight`, warns with `STALE_ROW`,
    drops `start_highlight`, and refuses to launch.
- **Typed search results (`crates/gui/farhelm-gui/src/launcher/view/search.rs:182-342, 420-431`,
  `crates/gui/farhelm-gui/src/launcher/view/history.rs:95-116`):**
  - `search_row_refusal` checks `Row::Recent` and `Row::Folder` but omits both `Row::Host { id, .. }` and
    `Row::Template(name)`, so `take_search_row` (`search.rs:277-300`) clears the search box (`self.search.set_text("")`
    at line 291) before `apply_search_row` discovers the host or template vanished. Check host existence AND template
    existence (`self.store.read(cx).collections().get::<LaunchTemplate>(name).is_none()`, matching `start.rs:681`) in
    `search_row_refusal` before clearing search text. Reuse `take_search_row`'s existing refusal path uniformly (which
    closes the results list via `self.close_search(window, cx)` so the narrow-window popover does not cover
    `launcher-search-refusal`, while keeping the typed search text intact).
  - In `search_enter` (`search.rs:268-271`), when `self.search_box.typed_choice` is `Some(picked)` and `picked` is no
    longer present in live `results.rows`, set `self.search_box.refusal = Some(STALE_ROW)`, clear `typed_choice`, keep
    the typed search text, close results (consistent with `take_search_row` so `launcher-search-refusal` stays visible
    in narrow windows), and return without falling through to Launch.
  - Keep `follow_history` (`crates/gui/farhelm-gui/src/launcher/view/history.rs:95-116`) dropping
    `self.history.answered` on installation change so an old host's history is never reused for a different
    installation. For host-history rows in `typed_choice`, bind `Option<Install>` alongside the chosen row (requiring
    matching `Install` in `same_search_choice`) rather than silently clearing `typed_choice = None` in
    `reset_host_history_highlight`, so an old choice can never match a different installation AND pressing Enter after a
    disconnect or install change warns with `STALE_ROW` instead of silently falling back to `results.default`. Clear
    `self.search_box.refusal` on typing, arrow navigation, or taking a valid choice.
- **Tests:** `crates/gui/farhelm-gui/src/launcher/search_view_tests.rs`, `recents_view_tests.rs`.

### Acceptance criteria

1. The stale-choice warning reads `"That choice is no longer available; choose a current result"` and displays in the
   existing inline refusal slot (`launcher-search-refusal`) below the command field.
2. In the empty command field, pressing Enter on a highlighted recent setup or template that disappeared (or clicking a
   stale recent setup or vanished template) shows the stale warning, drops the stale highlight, and does not fall
   through to Launch.
3. In typed search results, clicking a vanished host or vanished template result shows the stale warning and preserves
   the typed search text without selecting anything (closing results via `take_search_row`'s refusal path so the warning
   is visible in narrow windows too).
4. In typed search results, pressing Enter when any explicit keyboard pick (`typed_choice`: host, recent, template,
   folder, model, etc.) has disappeared from live results — including after a host disconnect or installation change —
   shows the stale warning, clears `typed_choice`, preserves the typed search text, and does not fall through to Launch.
5. Host installation changes continue to drop old host history (`self.history.answered`) and prevent an old
   choice/highlight (`Install`-bound) from ever matching a different installation's setup.
6. Tests cover all stale empty-pane, vanished-host click, vanished-template click, vanished explicit typed-choice Enter,
   and installation-change cases; and the TODO.md entry is removed.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Generalize the inline stale-choice warning to `"That choice is no longer available; choose a current result"`.
- Cover vanished templates and all other vanished explicit keyboard picks (not just hosts and recent setups).
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Reuse `crate::launcher::draft::Install` on both `SetupSource::Recent` and host-history `typed_choice` rather than
  clearing highlights silently on disconnect/install change, so install-change isolation holds AND Enter reliably shows
  `STALE_ROW` without falling through to Launch or `results.default`.
- Deliver as 1 focused GUI PR (`fix: warn and keep query when a launcher choice disappears`).

## Outline

1. `plan/explain-stale-launcher-choices/01-explain-stale-launcher-choices`: generalize `STALE_ROW` in
   `crates/gui/farhelm-gui/src/launcher/view/search.rs`, handle missing `start_highlight` (recent or template) and
   `RecentRefusal::Stale` in `search/start.rs`, validate `Row::Host` and `Row::Template` in `search_row_refusal` and
   missing `typed_choice` in `search_enter`, bind `Install` on `SetupSource::Recent` and host-history `typed_choice`
   while preserving `history.rs`'s history reset on install change, update any affected assertions and add regression
   tests in `search_view_tests.rs` and `recents_view_tests.rs`, add a `kind: fixed` changelog fragment, and remove the
   TODO.md entry ("Explain stale launcher choices"). `fix: warn and keep query when a launcher choice disappears`.

## Fallbacks

- None expected; if an existing test asserts that switching/disconnecting a host clears `typed_choice` before Enter
  specifically for host-history rows, check its intent before updating it and log the rationale as a DECISION.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-explain-stale-launcher-choices-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-explain-stale-launcher-choices-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted tests in `farhelm-gui` (`launcher::search_view_tests`, `launcher::recents_view_tests`,
`launcher::view_tests`), `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `dprint check` on
changed Markdown, and `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply
`.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live Farhelm
install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Explain stale launcher choices"), as root
  `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
