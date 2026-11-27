# Plan: Back off automatic retries when an exited shell tab's first close sweep fails

## Goal

Put an exited shell tab's failed first automatic cleanup sweep (`TabCloseSweep::BeforeEnd` under `only_if_dead == true`)
in `Supervisor::close_host_tab` (`crates/farhelm-supervisor/src/service/core/host_tabs.rs`) onto the existing 1 s to 60
s exponential backoff schedule instead of retrying on every 2-second supervisor tick.

Acceptance criteria:

- In `Supervisor::close_host_tab`, when `TabCloseSweep::BeforeEnd` fails and `tab.end_failed || only_if_dead`, Farhelm
  calls `mark_end_failed` so subsequent automatic reap retries follow the existing 1 s -> 2 s -> 4 s -> ... -> 60 s
  backoff schedule.
- A user's manual Close (or Stop) of a still-live tab (`only_if_dead == false`, `!tab.end_failed`) remains unmarked when
  `BeforeEnd` fails, so the background reaper never starts auto-closing a live tab and retries keep rooting at the live
  child.
- Doc comments on `HostTab::end_failed`, `HostTab::reap_due`, `mark_end_failed`, and `close_host_tab` accurately reflect
  that `end_failed` also covers automatic `BeforeEnd` sweep failures on finished tabs.
- A regression test verifies that when a finished shell tab's first automatic close sweep fails before ending its host,
  the tab is marked for backed-off retry rather than remaining immediately due on the next tick.
- The existing unreleased changelog fragment `releasing/changelog.d/tab-close-retry.md` (from PR #5087) is amended per
  `AGENTS.md`, "Changelog fragments".
- The last code PR removes the `Back off automatic retries when a finished shell tab's first close sweep fails.` entry
  from `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** From PR #5087: when a tab's shell exits on its own and Farhelm's first automatic
  cleanup fails during its initial process sweep (before ending the tab's terminal host), Farhelm retries it on every
  cleanup pass (~every 2 seconds) without backing off. Mark that failure path for backed-off retry in the same function
  and add a regression test.
- **Settled maintainer decisions:**
  - Put that missing exited-tab failure path on the existing 1 s to 60 s backoff schedule (keep the existing retry
    lifetime as-is, no retry-count cap) and add a regression test.
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** `SPEC.md` (shell tab automatic reaping and retry backoff), `SPEC_impl.md` (tab
  close retry), `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation"), and
  `plans/AGENTS.md`.
- **Planner-proposed choices (from fresh-context planning review):**
  - Reuse `HostTab::end_failed` / `mark_end_failed` rather than introducing a parallel `sweep_failed` timer. For
    `only_if_dead` tabs (which are already `finished()`), switching `anchor` to `TabReapAnchor::MarkerOnly` loses no
    live-child root (`Dead`/`Gone` already passes no root), and bypassing `Liveness::Unavailable` / re-adoption on a
    finished tab is desirable.
  - Single PR (`plan/shell-tab-first-close-sweep-backoff/01-backoff-first-sweep-failure`).

## Implementation outline

Single small PR in `crates/farhelm-supervisor/`:

1. In `crates/farhelm-supervisor/src/service/core/host_tabs.rs` (`Supervisor::close_host_tab`), change the
   `TabCloseSweep::BeforeEnd` failure branch from `if tab.end_failed` to `if tab.end_failed || only_if_dead`, calling
   `mark_end_failed(&entry.session.host_tabs, &tab.host)`.
2. Update doc comments on `HostTab::end_failed`, `HostTab::reap_due`, `mark_end_failed`, and the inline comment in
   `close_host_tab` so they accurately explain that `end_failed` covers failed automatic `BeforeEnd` sweeps on finished
   tabs as well as failed `end_tab_host` and `AfterEnd` sweeps, while manual closes of still-live tabs stay unmarked.
3. Add a focused regression test in `crates/farhelm-supervisor/src/service/core/host_tabs.rs` (or
   `crates/farhelm/tests/e2e/host_tabs.rs`, whichever cleanly injects a `BeforeEnd` sweep failure on an exited tab
   without sleeps) proving that after a first automatic `BeforeEnd` sweep failure, `reap_due()` is false until the
   1-second backoff elapses and later failures advance the backoff schedule, while a manual close failure of a live tab
   remains unmarked.
4. Amend `releasing/changelog.d/tab-close-retry.md` and remove the covered entry from `TODO.md`.

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
   `cargo xtask records path plan/shell-tab-first-close-sweep-backoff` (`plans/.agents/outside-files.md`, "Files outside
   the repository"). Immediately invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already
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
  `scripts/watchdog` (resolved with `realpath` at run time) using
  `farhelm-plan-shell-tab-first-close-sweep-backoff-watchdog.json` in the parent directory of the executing checkout
  (never inside the `plan/shell-tab-first-close-sweep-backoff` record directory). Watch the executing checkout root,
  `$HOME`, `/tmp`, the parent directory holding the status file, your scratch directory, and any worktree or build
  directory on another filesystem, at a 60-second interval with default alert levels. Follow `scode-build-goal`, "What
  the goal file must contain" (Resource watchdog) for verification, monitoring, `watchdog check` before launches and at
  boundaries, acting on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/shell-tab-first-close-sweep-backoff/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks
  `plan/shell-tab-first-close-sweep-backoff/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
  Restructure a PR that needs correcting rather than stacking a fix on top, including across earlier runs' open PRs.
  Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and
  validation"), `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and the
  docstring/comment rules with `scode-voice`. Run the targeted `terminal tabs in terminal hosts` selection from
  `.agents/validation.md` through `python3 scripts/record-test-run.py`, plus `cargo fmt`, `cargo clippy`,
  `dprint check`, `python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py`.
- **Review gate (explicit demand):** Before finishing any PR, run a fresh-context review on **Opus 5.5** at **high**
  effort (natively if available, otherwise via `$scode-harness-shellout`; do not hardcode a launch command) with this
  charter: review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
  against the goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and
  whether a simpler shape would), and idiomatic code for the languages involved; report findings to a named private file
  in your scratch directory, most severe first, with the file, the quoted code, what is wrong, and the smallest fix;
  edit nothing and touch no VCS state. Because this PR changes Rust tests, also supply `.agents/test-authoring.md`
  verbatim to the reviewer. Address findings before moving on.

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
