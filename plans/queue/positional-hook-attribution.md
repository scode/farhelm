# Attribute Grok and Codex hook reports to the process that ran the hook

Plan for the TODO.md entry "Attribute Grok and Codex hook reports to the process that ran the hook". Planned 2026-11-22.

## The goal

Farhelm admits an agent's conversation report only after attributing it to the session's agent process, by walking the
hook process's ancestry up to the session's terminal process. Grok's and Codex's attribution recognizes that agent
process by its executable file name; Claude's, Muse's and Antigravity's recognizes it by position (the process that ran
the hook, after skipping narrow `sh -c` trampolines), combined with the report pin: the first admitted reporting process
is pinned, and a later report from another process replaces the conversation only when it comes from an ancestor of the
pin.

The name rule broke Grok. On 2026-11-22 a live drift run on Grok 1.0.92 showed every report refused with "the session
pane looks like a Grok runtime that is not the recognised emitter": Grok's installer links `~/.grok/bin/grok` to a
versioned `~/.grok/downloads/grok-<version>-linux-x86_64`, and `is_grok_image` in
`crates/farhelm-supervisor/src/procs/grok.rs` accepts only `grok` and `grok-linux-x86_64`. Restart therefore cannot
resume Grok sessions on such installs. Claude's attribution was made positional for exactly this reason (#1137: Claude's
binary is named after its version and npm installs run under `node`; an earlier name-and-argument attempt, #830, was
abandoned as too complicated).

The maintainer's request, in their words: "lets see how we can do it in a way that's consistent across agents, lets set
some kind of rule how to handle this ... whatever we pick here let's see if we can make a standard rule that we can
default to until we have evidence it doesn't work for other agents". They chose position plus pin ("which is what you're
describing for claude code anti gravity, etc."), and for Codex: "I don't want to break codex again, but I do want to
bring codex to the same system as we now have with Claude".

So: move Grok and Codex to positional attribution with the report pin, write that down as the default rule for every
agent, and keep a name or launch-shape rule only where recorded evidence says the default does not work.

### Where it stands (from planning research at main 051e4495; line numbers approximate)

- `crates/farhelm-supervisor/src/procs/positional.rs`, `positional_corridor`: Claude, Muse and Antigravity. It returns
  process identities only (`PositionalAttribution { emitter, ancestors }`), not the emitter's chain link.
- `crates/farhelm-supervisor/src/procs/codex.rs`, `codex_corridor`: the innermost image named `codex` (`is_codex_image`,
  two platform copies in `crates/farhelm-supervisor/src/procs.rs`, near lines 1112 and 1708) is the emitter; only
  trampolines may sit between the reporter and it; Codex-named processes above it are its ancestors. #1945 added the pin
  for Codex (a launcher can share the name `codex`).
- `crates/farhelm-supervisor/src/procs/grok.rs`, `grok_corridor`: exactly one recognized Grok image, with `--no-leader`
  before the option boundary (`grok_has_owned_backend`); every other non-pane link must be a trampoline; the pane is
  accepted by position unless it looks like an unrecognized Grok (`looks_like_grok_runtime`, #2639).
- `crates/farhelm-supervisor/src/procs/omp.rs`: OMP verifies its launch shape (Bun bundle, compiled image, launchers)
  and uses `is_other_session_runtime` (near line 389), which therefore stays.
- The pin: `crates/farhelm-supervisor/src/agent_kind/report_pin.rs` (`ReportPin`, `PinDecision`, `ReportPin::judge`
  releasing a dead pin that is missing from the new chain), and in `crates/farhelm-supervisor/src/store.rs` `report_pin`
  and `admit_pinned_conversation` (which takes the replacement ownership version). Relaunch clears the pin; a missing
  pin judges Accept.
- `crates/farhelm-supervisor/src/service/core/vendor/codex.rs` (near lines 131-175 and 254-270) and
  `crates/farhelm-supervisor/src/service/core/vendor/positional.rs` (near lines 438-502 and 522-538) carry nearly the
  same pin-judging code: load and parse the pin, judge it, re-read liveness on Refuse, choose the replacement pin. They
  differ in that positional returns quietly on unknown liveness for Stop and has the `claude_pin_process_fault` test
  seam, while Codex returns an error.
- `crates/farhelm-supervisor/src/service/core/vendor/grok.rs`: no pin; admits through
  `admit_ownership_proven_conversation` (which OMP also uses, so it stays). `GrokLocator::merge_report`
  (`crates/farhelm-supervisor/src/agent_kind/grok.rs`, near lines 179-233) refuses enrichment (`UserPromptSubmit`,
  `Stop`) naming another conversation than the selected one, and requires a `SessionStart` for another conversation to
  be strictly newer.
- `crates/farhelm-supervisor/src/service/core.rs`, `report_conversation_for_kind` (near line 15582): an exhaustive match
  over agent kinds, with the no-wildcard lint, that routes each kind to its admission function and therefore its
  corridor.
- Per-vendor correction notifications: `crates/farhelm-supervisor/src/service/notifications.rs` (each kind has its own
  `...PinCorrected` kind and text; the stored strings are part of the once-per-launch key).
- Tests: corridor unit tests in `crates/farhelm-supervisor/src/procs.rs` (Codex refusal cases near lines 2828-3087, Grok
  cases near 3095-3200); `crates/farhelm/tests/e2e/codex_identity.rs` (including
  `a_shell_child_cannot_claim_a_pristine_codex_session`, which asserts the refusal this plan relaxes, and
  `nested_native_codex_reports_cannot_replace_the_foreground_conversation`); Grok in
  `crates/farhelm/tests/e2e/hook_identity.rs` (`native_grok_image`, `grok_selection_survives_...`).
- Documents describing the current rules: `SPEC.md` near line 4041 (Codex pins "the native Codex process") and near line
  4104 (Grok reports must come from one native `grok` process); `SPEC_impl.md` near lines 4180-4199 (innermost-`codex`
  rule, intermediary refusal, "Renamed native executables are not recognized", "Codex-image ancestors"), 4250-4256 (Grok
  attribution), 4290-4298 and 4341-4348; the agent map in `crates/farhelm-supervisor/src/agent_kind/mod.rs` module docs;
  user docs `website/src/content/docs/docs/agents/grok.md` (near 40-44), `.../agents/codex.md` (near 37-39, "must still
  be named `codex`"), `.../agents/agent-wrappers.md` (near 125-136), and `.../how-it-works/agent-hook-injection.md`
  (near 107 and 209).

### Acceptance criteria

1. Grok reports are attributed positionally: the emitter is the first non-trampoline ancestor of the hook reporter, and
   its argv must carry `--no-leader` before the option boundary (the existing `grok_has_owned_backend` check). No
   executable-name check remains for Grok; `is_grok_image`, `looks_like_grok_runtime` and the file's local `basename`
   are gone. A Grok whose image is `grok-<version>-linux-x86_64` (or any other name) is attributed.
2. Grok admission pins the first admitted emitter and judges later reports against the pin exactly as the other pinned
   vendors do, using the shared pin-judging code (Decisions, item 5). A Grok started inside the session that reported
   first is corrected by the real Grok's next `SessionStart`, with a new Grok correction notification. Grok keeps its
   locator merge, record verification and ownership version.
3. Codex reports are attributed positionally. For every chain the old corridor admitted, the emitter is the same
   process, so every working Codex setup keeps working: all Codex tests in `crates/farhelm/tests/e2e/codex_identity.rs`
   pass unchanged except `a_shell_child_cannot_claim_a_pristine_codex_session`, which is changed to assert the accepted
   relaxation (the shell child is pinned first; the root Codex's report corrects it with `CodexPinCorrected`).
   `is_codex_image` (both copies) is gone.
4. OMP's attribution is unchanged and documented as the evidence-backed exception.
5. The rule is written down: `SPEC_impl.md` states position plus pin as the default attribution for every agent kind,
   with a name or launch-shape rule only where recorded evidence shows the default does not work (OMP, with its reason);
   `SPEC.md`'s Grok and Codex passages match the new behavior; the agent map in
   `crates/farhelm-supervisor/src/agent_kind/mod.rs` and the doc comment of `report_conversation_for_kind` point at the
   rule. The accepted gaps (Decisions, item 7) are documented where the attribution rules are.
6. The user docs listed above describe the new behavior.
7. Live drift: `python3 -B scripts/run-harness-drift.py --harness grok --harness codex` reports PASSED for both on this
   machine's installed (and freshly upgraded) CLIs.
8. The last PR removes the TODO.md entry this plan covers.

## Decisions already made

1. Position plus pin is the standard attribution rule, chosen by the maintainer. It applies to Grok and Codex in this
   plan, and is the default for any future agent.
2. Grok keeps one Grok-specific check: the emitter was started with `--no-leader` before the option boundary. This is an
   argument check, not a name check, and Farhelm always passes `--no-leader`.
3. Codex moves to the same rule, with its own admission (its `/clear`, pending and Stop-confirmation rules) kept. The
   maintainer accepted the one relaxation this causes (questions page, 2026-11-22): a process that is not a narrow
   trampoline between the hook and Codex (a script, an interactive shell, a command Codex ran in its own shell) is
   accepted as the emitter instead of refused. Before the real Codex reports, such a process could set the conversation;
   the real Codex's report then corrects it with a notice. Claude, Muse and Antigravity already behave this way. A
   renamed native Codex and non-trampoline wrappers below Codex become supported.
4. OMP keeps its launch-shape corridor: it verifies how OMP was launched, not only which process reported. It is
   documented as the exception the rule allows.
5. The maintainer opted in to running the live Grok and Codex drift cases as acceptance, which spends a few real vendor
   turns (root `AGENTS.md` lists real agent tests as opt-in only; this is that opt-in).
6. Reviewer for every PR: a single fresh-context agent on `gpt-6.2-sol` at `high` effort (Review gate below).
7. Accepted gaps, to be documented rather than closed: a session that was running across the upgrade has a conversation
   but no pin, so a nested Grok's newer `SessionStart` can replace the parent's selection until the parent reports
   again; and if the parent's first report is lost, a nested child can select first. Both match what Claude, Muse and
   Antigravity accept. For an emitter that exists only for one hook run, the pin is dead by the next report, so it
   protects nothing for such setups (`ReportPin::judge` releases a dead pin missing from the chain); Grok's
   `--no-leader` check keeps refusing such emitters, Codex accepts them per item 3.

## Planner proposals (with reasons; challenge them through Scope reassessment if they turn out wrong)

- Share the pin-judging code. vendor/codex.rs and vendor/positional.rs already duplicate it, and Grok would be the third
  copy. Extract the pure replacement-pin choice onto `ReportPin` and the liveness-reading judge into `vendor/`, with the
  real differences (positional's quiet `Ok` on unknown liveness for Stop and its test seam, versus Codex's error) as a
  parameter or a returned value the caller interprets. Do not fold Grok or Codex into `PositionalReportPolicy`: those
  policies store plain ids at ownership version 0 with a yes/no record check, while Grok needs its merge, record
  verification and version 1, and Codex its own rules.
- Grok correction: a `SessionStart` from an ancestor of the pinned process (`PinDecision::Correct`) skips the
  strictly-newer ordering check against the child's selection and never inherits the child's transcript path, the way
  Codex corrects. Enrichment (`UserPromptSubmit`, `Stop`) from an ancestor while a child is pinned is acknowledged
  quietly (`Ok`) without changing the binding, as codex.rs and positional.rs answer reports they do not apply. Without
  this, `merge_report` would refuse the parent's reports and correction could happen only on a later `/new`.
- Add `GrokPinCorrected` to the notification kinds, mirroring the others (a variant, the `ALL` array, the exhaustive
  matches and a text constant).
- `positional_corridor` exposes the emitter's index or chain link alongside the identities, so Grok can check the
  emitter's argv; Claude, Muse and Antigravity ignore it.
- No new per-kind "attribution declaration": `report_conversation_for_kind` already forces every kind to choose an
  admission function and therefore a corridor. Writing the rule in the specs and pointing at it from the agent map and
  that function's doc comment is enough.
- Both behavior PRs are user-visible: the Grok PR is `fix` (Restart works again), the Codex PR is `fix` (more installs
  and wrappers are captured; a child reporting first now gets a correction notice instead of a refusal). Each leaves a
  changelog fragment.

## Outline

A linear stack, bookmarks `plan/positional-hook-attribution/<nn>-<short-name>`:

1. `01-shared-pin-judge`: `refactor:` extract the shared pin-judging code from vendor/codex.rs and vendor/positional.rs
   with no behavior change; existing tests pass unchanged.
2. `02-grok-positional`: `fix:` Grok moves to `positional_corridor` plus the `--no-leader` argv check and gains the pin
   through the shared code, the correction rule above and `GrokPinCorrected`; Grok-specific helpers removed; corridor
   and admission unit tests, and the Grok e2e tests in `crates/farhelm/tests/e2e/hook_identity.rs` updated (a versioned
   image name attributed; a nested Grok that reports first corrected by the parent's `SessionStart`); `SPEC.md`,
   `SPEC_impl.md` and `website/src/content/docs/docs/agents/grok.md` updated for Grok; changelog fragment.
3. `03-codex-positional`: `fix:` Codex moves to `positional_corridor`; `is_codex_image` removed; the refusal unit tests
   in `crates/farhelm-supervisor/src/procs.rs` and `a_shell_child_cannot_claim_a_pristine_codex_session` rewritten for
   the accepted relaxation; every other Codex test unchanged; `SPEC.md`, `SPEC_impl.md`, `codex.md`, `agent-wrappers.md`
   and `agent-hook-injection.md` updated for Codex; changelog fragment.
4. `04-attribution-rule`: `docs:` the standard rule in `SPEC_impl.md` (default for every kind, the evidence bar for an
   exception, OMP as the exception, the accepted gaps), pointers from the agent map and `report_conversation_for_kind`;
   removes this plan's TODO.md entry. It may instead be folded into PR 2 if writing the rule there reads better; either
   way the TODO entry goes in the last PR.

The live drift run (acceptance 7) happens once the stack is complete, from its tip.

## Fallbacks

- If the live Grok drift case still fails after PR 2 for a reason other than attribution (a new trust prompt, a TUI
  change in the drift test itself), fix the drift test within this plan when the fix is in
  `crates/farhelm/tests/e2e/real_agent_capture/drift.rs` and small; otherwise record it in the report and keep the
  attribution work. Investigate a real CLI with `scripts/agent-tui.py` per `.agents/real-agent-tui.md`.
- If a currently passing Codex test other than `a_shell_child_cannot_claim_a_pristine_codex_session` fails under
  positional attribution, that contradicts acceptance 3: stop the Codex PR, keep PRs 1 and 2, and block with the failing
  scenario as the question.
- If a vendor CLI is not logged in, so its live drift case reports COULD NOT RUN, record that in the report as missing
  live evidence; do not sign in on the maintainer's behalf.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-positional-hook-attribution-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-positional-hook-attribution-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Rebuild `farhelm-fixtures` before running e2e tests. Targeted tests: the corridor and pin unit tests in
`farhelm-supervisor` (`procs`, `agent_kind::report_pin`, `agent_kind::grok`, the vendor admission and notification
tests), and the e2e `codex_identity` and `hook_identity` modules; `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, `dprint check` on changed
Markdown, the website build for the docs pages (`website/AGENTS.md`; an unattended run starts no preview server), and
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to
every test you add or change. The live drift run is
`python3 -B scripts/run-harness-drift.py --harness grok --harness codex` (it upgrades both CLIs first and records each
case; `docs/harness-drift-tests.md`). Never take a state-changing action against the live Farhelm install on this
machine (root `AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Attribute Grok and Codex hook reports to the
  process that ran the hook"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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

Grok conversation tracking has been fixed. It has been broken at least for a while. It is now on mechanism less likely
to break in the future. Codex has also been made a little more resilient to certain environments or potential changes in
codex agent binary naming.
