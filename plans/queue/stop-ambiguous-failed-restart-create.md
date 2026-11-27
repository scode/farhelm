# Plan: Clarify Stop refusal message on unconfirmed launches and Restarts

## Goal

Keep the uniform Stop refusal for sessions whose latest launch was never confirmed and that have no main terminal
(`terminal_less_launch_may_be_live`), and reword the refusal message, spec wording, comments, and unreleased changelog
fragment so they accurately describe both an unconfirmed initial start and an ambiguous failed Restart and clearly state
that nothing (including open terminal tabs) was stopped.

Acceptance criteria:

- `UNCONFIRMED_LAUNCH_STOP_REFUSAL` in `crates/farhelm-supervisor/src/service/handlers.rs` no longer inaccurately claims
  "there is no terminal for Stop to stop" or implies only an initial start can hit it; it states plainly that the
  session's latest launch was never confirmed, that Stop cannot safely tell what is running so nothing was stopped
  (including open terminal tabs), and directs the user to Delete (or Restart, if offered).
- Doc comments in `crates/farhelm-supervisor/src/service/handlers.rs` and
  `crates/farhelm-supervisor/src/service/core.rs` (`terminal_less_launch_may_be_live`) match the accurate wording.
  Delete's own guard refusal (`still_alive_for_delete` in `core.rs`) is left untouched.
- `SPEC.md` (the Stop section describing unconfirmed launches) drops the inaccurate "there is nothing for Stop to stop"
  phrasing and accurately covers ambiguous failed Restart, post-terminal failed create, and open terminal tabs staying
  untouched.
- Tests asserting substrings or equality on the refusal message (`crates/farhelm-supervisor/src/service/handlers.rs` and
  `crates/farhelm/tests/e2e/host_creates.rs`) are updated in lockstep.
- The existing unreleased changelog fragment `releasing/changelog.d/stop-unconfirmed-start.md` (from PR #5091) is
  amended to remove the inaccurate "With nothing yet running that Stop could stop" phrasing.
- The last code PR removes the
  `Decide whether Stop should keep refusing sessions after an ambiguous failed Restart or post-terminal failed create (and review refusal wording and tab closing).`
  entry from `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** From PR #5091: Stop now refuses any session whose start was never confirmed,
  including ambiguous failed Restarts and post-terminal failed creates, where open shell tabs are also not closed and
  the refusal message ("there is no terminal for Stop to stop") is inaccurate. Decide whether to keep the uniform
  refusal (and optionally reword the refusal message) or let Stop act in these cases.
- **Settled maintainer decisions:**
  - Keep the uniform Stop refusal on `terminal_less_launch_may_be_live` (do not teach Stop to fence launch records or
    settle unconfirmed launches) and reword the refusal message accurately to cover both unconfirmed creates and
    ambiguous failed Restarts and note that nothing (including open terminal tabs) was stopped.
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** `SPEC.md` (Stop), `SPEC_impl.md` (Stop), `AGENTS.md` ("Conventional Commits",
  "Changelog fragments", "Tests and validation"), and `plans/AGENTS.md`.
- **Planner-proposed choices (from fresh-context planning review):**
  - Keep Delete's guard refusal (`still_alive_for_delete` in `core.rs`) untouched.
  - Single PR (`plan/stop-ambiguous-failed-restart-create/01-reword-stop-refusal`).

## Implementation outline

Single small PR with no runtime state-machine changes:

1. In `crates/farhelm-supervisor/src/service/handlers.rs`, reword `UNCONFIRMED_LAUNCH_STOP_REFUSAL` (and its doc comment
   and inline comment near `handle_stop_session`), for example: _"this session's latest launch was never confirmed, so
   Stop cannot tell what is running and nothing was stopped (including any open terminal tabs); its agent may still be
   starting. Delete the session to end whatever its launch left, or Restart it if Restart is offered"_.
2. Update the doc comment on `terminal_less_launch_may_be_live` in `crates/farhelm-supervisor/src/service/core.rs`.
   Leave `still_alive_for_delete` untouched.
3. Update `SPEC.md` (the Stop paragraph on unconfirmed starts/Restarts) to remove "there is nothing for Stop to stop"
   and accurately describe why Stop refuses without closing open terminal tabs. Check `SPEC_impl.md` (already accurate;
   leave unchanged unless needed).
4. Update the docstring and assertions in `crates/farhelm-supervisor/src/service/handlers.rs` and
   `crates/farhelm/tests/e2e/host_creates.rs` (`message.contains(...)`) to match the updated constant.
5. Amend `releasing/changelog.d/stop-unconfirmed-start.md` to remove `"With nothing yet running that Stop could stop"`,
   and remove the covered entry from `TODO.md`.

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
   `cargo xtask records path plan/stop-ambiguous-failed-restart-create` (`plans/.agents/outside-files.md`, "Files
   outside the repository"). Immediately invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md`
   already exists, read all of it and resume from where the previous session left off, cross-checking against GitHub and
   VCS state (`plans/.agents/executing.md`, "Executing one plan", step 7); otherwise start fresh.

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
  `farhelm-plan-stop-ambiguous-failed-restart-create-watchdog.json` in the parent directory of the executing checkout
  (never inside the `plan/stop-ambiguous-failed-restart-create` record directory). Watch the executing checkout root,
  `$HOME`, `/tmp`, the parent directory holding the status file, your scratch directory, and any worktree or build
  directory on another filesystem, at a 60-second interval with default alert levels. Follow `scode-build-goal`, "What
  the goal file must contain" (Resource watchdog) for verification, monitoring, `watchdog check` before launches and at
  boundaries, acting on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/stop-ambiguous-failed-restart-create/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks
  `plan/stop-ambiguous-failed-restart-create/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
  Restructure a PR that needs correcting rather than stacking a fix on top, including across earlier runs' open PRs.
  Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and
  validation"), `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and the
  docstring/comment rules with `scode-voice`. Run the targeted supervisor Stop unit test and the `host_creates`
  Stop-refusal e2e test through `python3 scripts/record-test-run.py`, plus `cargo fmt`, `cargo clippy`, `dprint check`,
  `python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py`.
- **Review gate (explicit demand):** Before finishing any PR, run a fresh-context review on **Opus 5.5** at **high**
  effort (natively if available, otherwise via `$scode-harness-shellout`; do not hardcode a launch command) with this
  charter: review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
  against the goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and
  whether a simpler shape would), and idiomatic code for the languages involved; report findings to a named private file
  in your scratch directory, most severe first, with the file, the quoted code, what is wrong, and the smallest fix;
  edit nothing and touch no VCS state. Because this PR updates Rust tests, also supply `.agents/test-authoring.md`
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
