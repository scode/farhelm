# Plan: Update release brick-test instructions for the inline Welcome view

## Goal

Update the macOS release brick-test instructions, runbook, and unit tests under `releasing/mac-vm-test/` so candidate
fresh installs and post-upgrade launches no longer expect the removed startup Welcome modal card ("Got it"), while
previous-release baselines handle the old Welcome card conditionally.

Acceptance criteria:

- Candidate `app-start` and candidate `update-relaunch` instructions (`releasing/mac-vm-test/guest_workflow.py` and
  `releasing/mac-vm-test/checkpoint_contract.py`) no longer instruct the guest agent to dismiss a startup Welcome modal
  card.
- Plain-envelope instructions in `releasing/mac-vm-test/guest_workflow.py` preserve the
  `"Do not select Send feedback now or begin P2 early."` guard for `app-start`, `old-fixture`, and `update-relaunch`.
- Previous-release baseline `old-fixture` instructions handle the Welcome card conditionally
  (`dismiss welcome with got it if present` / `"If the ordinary welcome card appears, dismiss it with got it..."`) so
  pre-card baselines, card-era baselines, and future baselines all work.
- Announcements' own **Got it** dismissal (`checkpoint_contract.py` and `RECIPE.md`) is left untouched.
- `releasing/mac-vm-test/BRICK-TEST.md` reflects that candidate builds show an inline Welcome view in the empty main
  pane (no startup modal to dismiss), while older previous-release baselines may still show a Welcome card to dismiss
  with **got it** if present.
- Unit tests in `releasing/mac-vm-test/test_guest_workflow.py` pin the `app-start`, `old-fixture`, and `update-relaunch`
  instruction text across both managed and plain envelopes.
- The last code PR removes the `Update release brick-test instructions for the removed startup Welcome card.` entry from
  `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** PRs #5012 and #5018 replaced the startup Welcome modal card (dismissed with "Got
  it") with an inline Welcome view in the empty main pane. Remove the unconditional Welcome dismissal from candidate
  `app-start`, update `BRICK-TEST.md` (keeping conditional handling only for older previous-release baselines), and
  update the brick-test unit tests under `releasing/mac-vm-test/`.
- **Settled maintainer decisions:**
  - Remove Welcome modal dismissal from both candidate `app-start` and candidate `update-relaunch`, and make
    `old-fixture` conditional (`dismiss welcome with got it if present`).
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** Read `releasing/AGENTS.md` before editing under `releasing/` (`AGENTS.md`,
  "Instructions by path"), plus `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation") and
  `plans/AGENTS.md`.
- **Planner-proposed choices:**
  - Single PR (`plan/brick-test-inline-welcome/01-update-welcome-instructions`), adding assertions directly to
    `CheckpointInstructionsTests` in `releasing/mac-vm-test/test_guest_workflow.py`.

## Implementation outline

Single small PR touching only `releasing/mac-vm-test/` and `TODO.md` (read `releasing/AGENTS.md` first):

1. `releasing/mac-vm-test/guest_workflow.py`:
   - In `managed_instructions()` for `'app-start'`, drop `dismiss welcome with got it, `.
   - In `managed_instructions()` for `'old-fixture'`, change `dismiss welcome with got it,` to
     `dismiss welcome with got it if present,`.
   - In `instructions()`, split the `('app-start', 'old-fixture', 'update-relaunch')` branch so `'old-fixture'` keeps
     both the conditional Welcome-card dismissal and `"Do not select Send feedback now or begin P2 early."`, while
     `'app-start'` and `'update-relaunch'` keep only `"Do not select Send feedback now or begin P2 early."`.
2. `releasing/mac-vm-test/checkpoint_contract.py`:
   - In `update_instructions()`, drop `and dismiss welcome with got it if present` for candidate `update-relaunch`.
   - Leave `announcements_instructions()` (Announcements dialog **Got it**) untouched.
3. `releasing/mac-vm-test/BRICK-TEST.md`:
   - Update the startup Welcome paragraph under `## Perform the product checks` to state that candidate builds show an
     inline Welcome view in the empty main pane with no startup modal to dismiss, while older previous-release baselines
     may still show a startup Welcome card to dismiss with **got it** if present.
4. `releasing/mac-vm-test/test_guest_workflow.py`:
   - Add unit tests in `CheckpointInstructionsTests` pinning both `managed=True` and `managed=False` instructions for
     `app-start`, `old-fixture`, and `update-relaunch`.
5. Remove the covered entry from `TODO.md`. (Internal test/doc change; no user-facing changelog entry, or `kind: none`
   if a required commit type is used.)

## Scope reassessment

Before implementing a substantial departure from the outline — or after repeated corrective reviews of the same
component — run a fresh-context review at the executing agent's own model and effort with this charter:

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

Supply the request, settled decisions, outline, current diff, and proposed departure (what changed, why it is necessary,
and which simpler alternative was ruled out). A routine fork within the agreed scope is decided after reassessment and
logged as `DECISION`. Any material scope expansion, weakened guarantee, or omitted required behavior follows the
unattended fallback below.

## Checkout and resume protocol

1. **Checkout check:** Run inside the checkout the executing session runs in, which must be the nearest Git worktree or
   jj workspace containing the working directory (a `.git` file counts as well as `.git`). Confirm a remote points to
   `scode/farhelm` (compared as host, owner, and name). If not, stop before the resume protocol, write nothing, and
   report the mismatch. Every project path in this plan is relative to that checkout root.
2. **Resume protocol:** Resolve the plan's working log directory with
   `cargo xtask records path plan/brick-test-inline-welcome` (`plans/.agents/outside-files.md`, "Files outside the
   repository"). Immediately invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already
   exists, read all of it and resume from where the previous session left off, cross-checking against GitHub and VCS
   state (`plans/.agents/executing.md`, "Executing one plan", step 7); otherwise start fresh.

## Execution mode and resource watchdog

- **No-workhorse mode:** This is an explicit no-workhorse choice. Never send any unit of your own decomposition to a
  weaker model or lower effort than your own, and never invoke, load, or follow `scode-galaxy-brain` or
  `scode-model-routing` at any point (if `scode-galaxy-brain` is already active, stop it). Same-model, same-effort
  sub-agents are allowed at your judgment when likely to finish sooner without costing quality (using separate worktrees
  or jj workspaces if concurrent writers would conflict, while you retain ownership of `jjstack` and every PR). The
  sub-agents required by `plans/AGENTS.md`, "Sub-agents this file requires", the per-PR review gate below, and any scope
  reassessment review are exempt from any no-delegation rule. Launch sub-agents natively where supported, or through
  `$scode-harness-shellout` when the demanded model is unavailable natively.
- **Resource watchdog:** Right after the resume protocol and before any delegation, start the `scode-build-goal` skill's
  `scripts/watchdog` (resolved with `realpath` at run time) using `farhelm-plan-brick-test-inline-welcome-watchdog.json`
  in the parent directory of the executing checkout (never inside the `plan/brick-test-inline-welcome` record
  directory). Watch the executing checkout root, `$HOME`, `/tmp`, the parent directory holding the status file, your
  scratch directory, and any worktree or build directory on another filesystem, at a 60-second interval with default
  alert levels. Follow `scode-build-goal`, "What the goal file must contain" (Resource watchdog) for verification,
  monitoring, `watchdog check` before launches and at boundaries, acting on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/brick-test-inline-welcome/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks `plan/brick-test-inline-welcome/<nn>-<short-name>`.
  Keep PRs as drafts; never mark ready and never merge. Restructure a PR that needs correcting rather than stacking a
  fix on top, including across earlier runs' open PRs. Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and
  validation"), `releasing/AGENTS.md`, `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`,
  and the docstring/comment rules with `scode-voice`. Run the portable brick-test unittest suite
  (`releasing/mac-vm-test`, per `.agents/validation.md`) through `python3 scripts/record-test-run.py`, `dprint check`,
  and `python3 scripts/check-agents-md.py` if any heading or instruction reference changed.
- **Review gate (explicit demand):** Before finishing any PR, run a fresh-context review on **Opus 5.5** at **high**
  effort (natively if available, otherwise via `$scode-harness-shellout`; do not hardcode a launch command) with this
  charter: review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
  against the goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and
  whether a simpler shape would), and idiomatic code for the languages involved; report findings to a named private file
  in your scratch directory, most severe first, with the file, the quoted code, what is wrong, and the smallest fix;
  edit nothing and touch no VCS state. Because this PR changes tests, also supply `.agents/test-authoring.md` verbatim
  to the reviewer. Address findings before moving on.

## Decision logging and unattended fallback

- Log consequential implementation choices in `log.md` as they happen with a scannable `DECISION` label and the
  alternatives considered.
- Never ask the maintainer anything in conversation — no direct questions, no harness question prompts, no optional
  next-step offers, and no waiting for confirmation (`plans/AGENTS.md`, "Unattended means no questions"). Decide routine
  implementation forks within the agreed scope, log them as `DECISION`, and carry on. Remove unnecessary
  planner-invented machinery when requested behavior and repository constraints still hold. If a material scope
  expansion, weakened guarantee, omitted required behavior, or decision only the maintainer can make comes up, block per
  `plans/.agents/executing.md`, "Executing one plan" (step 10, Blocking). Put things the maintainer should know and
  possible follow-ups in the report.

## Done criterion

Every PR in the stack exists as a draft PR on GitHub, satisfies the acceptance criteria (including removing the
`TODO.md` entry in the last code PR), passes its chosen validation and its Opus 5.5 high review gate, and satisfies the
latest `## Decisions` entry, if any. Delivering the report is the executor's closing step (`plans/.agents/executing.md`,
"Executing one plan", step 11), not part of this goal.

## Release note

No release note needed.
