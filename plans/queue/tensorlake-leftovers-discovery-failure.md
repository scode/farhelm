# Keep all Tensorlake sandbox leftovers visible when discovery fails

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Keep all sandbox leftovers visible when Tensorlake discovery fails.** During unfinished cleanup, a failed refresh
> can replace the previously listed sandboxes with the cleanup rows alone. Have the helm report the discovery failure
> alongside the current cleanup snapshot, and keep the last discovered rows in Settings. Use the sandbox ID to show one
> row when cleanup supersedes discovery. A known empty cleanup snapshot after the last cleanup finishes should remove
> its old **Cleaning up…** or **Retry cleanup** row even during the outage. Keep the existing reply bound, and
> distinguish a discovery failure from an individual cleanup error in Settings. Cover refresh, cleanup completion during
> the outage, and recovery.

In product terms: in Settings → Tensorlake → **Sandboxes not on this helm**, never wipe out previously discovered
still-billing sandboxes when listing fails during an active cleanup, and never leave a finished cleanup's **Cleaning
up…** or **Retry cleanup** row stuck on screen during an outage. Return any discovery failure alongside the helm's
current cleanup snapshot (even when empty), merge kept discovered rows with cleanup rows by sandbox ID without
resurrecting deleted sandboxes, keep cleanup rows' own status text intact, and display the list-level discovery failure
above the kept rows with an explicit lead-in (`"Could not refresh sandbox list: …"`), clearly separated from per-row
cleanup errors and Delete errors.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Proto & helm (`crates/farhelm-proto/src/tensorlake.rs:135-156`,
  `crates/farhelm-helm/src/tensorlake/orphans.rs:112-175, 311-317`):**
  - `GET /api/tensorlake/orphans` currently returns `Vec<OrphanSandbox>` on 200 or plain-text error on 502/504. When
    discovery fails while `unfinished` cleanups exist, `orphan_sandboxes_by` overwrites each cleanup row's `state` with
    `"Sandbox list unavailable: <error>"` and returns 200 with only those rows; when the last cleanup finishes during
    the outage, `unfinished` becomes empty and it flips to 502 with no cleanup snapshot.
  - Replace the response shape with a structured
    `OrphanSandboxList { discovery: OrphanDiscovery, cleanups: Vec<OrphanSandbox> }` where `OrphanDiscovery` is a tagged
    two-variant enum (`Ok { sandboxes: Vec<OrphanSandbox> }` / `Failed { reason: String }`) so invalid `(Some, Some)` /
    `(None, None)` states are unrepresentable. Return `200 OK` whenever the helm can read its cleanup snapshot (keeping
    the existing 20-second `HOST_REPLY_BOUND` on discovery). Sort each vec by `name` on the helm, and keep cleanup rows'
    own `"Cleanup in progress"` / `"Cleanup unfinished"` state untouched.
  - Note: only Settings (`tensorlake_sandboxes.rs`), tests, and `goldens/settings.rs` call `/api/tensorlake/orphans`,
    and the desktop embeds its own helm. Also update
    `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs:1552, 1579` (calls `f.tensorlake.orphan_sandboxes()`)
    and `crates/gui/farhelm-gui/src/settings/recipe_tests.rs:786` (mocks `/api/tensorlake/orphans`).
- **Desktop GUI (`crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs:89-235, 287-322, 343-430, 443-531`):**
  - Track `last_discovered: Vec<OrphanSandbox>` separately from `cleanups: Vec<OrphanSandbox>`. Clear **both** in
    `follow_account` (`tensorlake_sandboxes.rs:162-167`) when the key clears, and check
    `!self.last_discovered.is_empty() || !self.cleanups.is_empty()` in `drawn()` (line 335).
  - **Unconditional non-resurrection rule:** On **every** response (healthy or failed), unconditionally remove any
    sandbox ID present in `cleanups` from `last_discovered` (and also remove the ID from `last_discovered` on 204
    Delete). Never leave an ID that entered `cleanups` sitting inside `last_discovered`, or a sandbox that was still
    listed while `Cleanup in progress` would resurrect once `cleanups` empties during a subsequent outage!
  - On `OrphanDiscovery::Ok { sandboxes }`: replace `last_discovered` with `sandboxes` (minus any ID present in
    `cleanups`), update `cleanups`, clear `discovery_error`, and advance `checked_at`.
  - On `OrphanDiscovery::Failed { reason }`: keep `last_discovered` (removing any ID present in `cleanups`), replace
    `cleanups` from the snapshot (so when the last cleanup finishes and `cleanups` becomes empty, its old **Cleaning
    up…** / **Retry cleanup** row disappears cleanly), record `discovery_error`, and do **not** advance `checked_at`.
  - **Merged row order:** Render `last_discovered` in order (excluding any ID superseded by `cleanups`) followed by
    `cleanups` in order — preserving the existing row order in `goldens/settings.rs:555-565` and
    `tensorlake_sandbox_tests.rs:281-309` so `settings-tensorlake-sandboxes` golden pixels stay unchanged!
  - Render the list-level discovery failure (`tensorlake-sandboxes-error`) **above** the kept rows with an explicit
    lead-in (`"Could not refresh sandbox list: <reason>"`), keeping per-row cleanup errors inside each row and Delete
    errors (`tensorlake-sandboxes-delete-error`) distinct.
  - If both `last_discovered` and `cleanups` are empty during a discovery failure, show the discovery failure without
    claiming `"None found"`.
- **Specs, tests & goldens:**
  - `SPEC.md:565-576`, `SPEC_impl.md:293-298, 7009-7038`.
  - `crates/farhelm-helm/src/tensorlake/orphans.rs:353-658`, `crates/farhelm-helm/src/tensorlake/tests.rs:1335-1556`,
    `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs:1552, 1579`.
  - `crates/gui/farhelm-gui/src/settings/tensorlake_sandbox_tests.rs`, `tensorlake_tests.rs`, `recipe_tests.rs:786`, and
    `crates/gui/farhelm-gui/src/goldens/settings.rs:493, 549-595` (update mocked JSON fixtures to `OrphanSandboxList`).

### Acceptance criteria

1. `GET /api/tensorlake/orphans` returns `OrphanSandboxList` (`discovery` + `cleanups`) within the existing 20-second
   reply bound, keeping cleanup rows' own state intact even on discovery failure or timeout and returning an empty
   `cleanups` snapshot cleanly when the last cleanup finishes during an outage.
2. Settings keeps previously discovered rows during a discovery failure, deduplicates by sandbox ID so cleanup
   supersedes discovery without resurrecting a deleted sandbox when cleanup finishes during the same outage (stripping
   any cleanup ID from `last_discovered` unconditionally on every response), and removes completed **Cleaning up…** /
   **Retry cleanup** rows during an outage.
3. Settings renders the list-level discovery failure **above** the kept rows with an explicit lead-in
   (`"Could not refresh sandbox list: …"`), clearly distinct from per-row cleanup errors and Delete refusal errors.
4. Tests cover refresh failure with kept rows, cleanup completion during an outage (no stale cleanup row and no
   resurrection, including when a sandbox was still listed during cleanup before the outage), key-clear reset, and full
   recovery once discovery succeeds again.
5. `SPEC.md` and `SPEC_impl.md` are updated, golden fixtures/images are updated if affected, and the TODO.md entry is
   removed.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Show the list-level discovery error **above** the kept rows with an explicit lead-in
  (`"Could not refresh sandbox list: …"`), clearly separated from per-row cleanup errors and Delete errors.
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `AGENTS.md`, "Code rules": keep Tensorlake orphan handling in the Tensorlake provider module (and keep it separate
  from `GET /api/container-orphans`, which has no helm-side snapshot cleanup state).
- `.agents/validation.md` for GUI goldens (`check-gui-goldens.py` + `gui-goldens.py publish`).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Use a structured `OrphanSandboxList { discovery: OrphanDiscovery, cleanups: Vec<OrphanSandbox> }` response shape so
  invalid states are unrepresentable and Settings never guesses provenance from `row.cleanup.is_some()`.
- Deliver as **1 cohesive PR** so the wire-format change between the embedded helm and `farhelm-gui` is atomic and never
  leaves an intermediate commit where Settings fails to decode `/api/tensorlake/orphans`.

## Outline

1. `plan/tensorlake-leftovers-discovery-failure/01-keep-leftovers-on-discovery-failure`: add `OrphanSandboxList` and
   `OrphanDiscovery` in `farhelm-proto/src/tensorlake.rs`, update `crates/farhelm-helm/src/tensorlake/orphans.rs` to
   return discovery outcome + cleanup snapshot under the existing 20-second bound (keeping cleanup row states intact and
   returning empty `cleanups` once cleanup finishes), update
   `crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs` to track `last_discovered` and `cleanups` separately
   with unconditional non-resurrection, drop finished cleanup rows during an outage, render the discovery error above
   kept rows with `"Could not refresh sandbox list: …"`, update `SPEC.md` / `SPEC_impl.md`, update all helm/GUI tests
   and mocks (`orphans.rs`, `tests.rs`, `tensorlake_add_tests.rs`, `tensorlake_sandbox_tests.rs`, `tensorlake_tests.rs`,
   `recipe_tests.rs`, `goldens/settings.rs`), add a `kind: fixed` changelog fragment, and remove the TODO.md entry
   ("Keep all sandbox leftovers visible when Tensorlake discovery fails").
   `fix: keep Tensorlake leftovers visible when discovery fails`.

## Fallbacks

- If a non-2xx transport/helm error occurs on `GET /api/tensorlake/orphans` (e.g. helm disconnected), keep the existing
  fallback behavior in `tensorlake_sandboxes.rs` (retain currently shown rows and display the error above them with the
  same discovery-failure lead-in).
- If `scripts/gui-goldens.py publish` is needed and cannot reach the private goldens storage repository, block with the
  exact error rather than committing a broken manifest.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tensorlake-leftovers-discovery-failure-log.md` in the parent directory of the checkout you run in, derived
as that section says, and the watchdog's status file is
`farhelm-plan-tensorlake-leftovers-discovery-failure-watchdog.json` beside it. Resolve both to absolute paths before you
start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`, "Agent
scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted tests in `farhelm-proto`, `farhelm-helm` (`tensorlake::orphans`, `tensorlake::tests`,
`provisioning::tensorlake_add_tests`), and `farhelm-gui` (`settings::tensorlake_sandbox_tests`,
`settings::tensorlake_tests`, `settings::recipe_tests`), `check-gui-goldens.py` (with `--scene 'settings-tensorlake*'`
or full check if visuals change), `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`dprint check` on changed Markdown, and `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares
it. Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the
live Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Keep all sandbox leftovers visible when
  Tensorlake discovery fails"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
