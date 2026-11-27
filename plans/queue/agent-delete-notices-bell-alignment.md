# Plan: Align spec, agent instructions, and comments on agent-Delete notices

## Goal

Keep the helm posting every cleanup warning produced by `farhelm agent delete` (including both leftover managed-checkout
warnings and unconfirmed scoped-Podman-container-stop warnings) to the desktop app's notices bell, and widen `SPEC.md`,
`SPEC_impl.md`, live agent instructions, comments/logs, and the unreleased changelog fragment so they say "Delete
notice" / cleanup notice instead of "checkout notice" narrowly.

Acceptance criteria:

- `crates/farhelm-helm/src/agent_requests.rs` continues posting every `outcome.notice` from `farhelm agent delete` to
  the notices bell without protocol or filtering changes.
- `SPEC.md` (Managed checkouts, Navigation rail notices bell, and `farhelm agent delete`) describes agent-requested
  Delete putting any Delete/cleanup notice (leftover checkout or unconfirmed Podman container stop) in the notices bell.
- `SPEC_impl.md` (late Delete / notices bell, `farhelm agent delete`, and environment Delete notice sections where
  applicable) matches.
- `crates/farhelm/src/agent_instructions.rs` (the live instruction text injected into agent sessions for
  `farhelm agent delete`) is widened from "checkout notice" to "Delete notice" / cleanup notice (and any tests pinning
  that exact string are updated).
- Code comments, test comments, and log/panic messages in `crates/farhelm-helm/src/agent_requests.rs`,
  `crates/farhelm-helm/src/notices.rs`, and `crates/farhelm/tests/e2e/agent_relay.rs` no longer describe agent-Delete
  bell notices as checkout-only.
- The existing unreleased changelog fragment `releasing/changelog.d/agent-delete-checkout-notice.md` (from PR #5088) is
  amended to cover all Delete cleanup warnings rather than checkouts alone.
- The last code PR removes the `Align SPEC.md and code on which agent-requested Delete warnings go to the notices bell.`
  entry from `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** From PR #5088: the code posts every notice `farhelm agent delete` produces to the
  notices bell, whereas `SPEC.md`'s agent-Delete rule says only leftover-checkout notices go to the bell. Decide whether
  to widen `SPEC.md` to "Delete notice" or narrow the code to checkout notices only.
- **Settled maintainer decisions:**
  - Keep posting all agent-Delete cleanup warnings (leftover checkouts and unconfirmed scoped-Podman-container stops) to
    the notices bell, and widen `SPEC.md`, `SPEC_impl.md`, comments, and the unreleased changelog fragment to match.
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** `SPEC.md`, `SPEC_impl.md`, `AGENTS.md` ("Conventional Commits", "Changelog
  fragments", "Tests and validation"), and `plans/AGENTS.md`.
- **Planner-proposed choices (from fresh-context planning review):**
  - Also align the three live narrow `"checkout notice"` sites spotted during planning review:
    `crates/farhelm/src/agent_instructions.rs:123`, `SPEC_impl.md:7712` (and check `:6734`), and
    `crates/farhelm-helm/src/notices.rs:553` (plus `crates/farhelm/tests/e2e/agent_relay.rs:479`).
  - Single PR (`plan/agent-delete-notices-bell-alignment/01-widen-delete-notice-wording`).

## Implementation outline

Single small PR with no runtime behavior change beyond updating the injected agent instruction line:

1. Update `SPEC.md` in its Managed checkouts, Navigation rail notices bell, and `farhelm agent delete` sections so
   agent-requested Delete is specified as posting any Delete cleanup notice (such as a leftover checkout or an
   unconfirmed Podman container stop) to the notices bell.
2. Update `SPEC_impl.md` where late Delete notices and `farhelm agent delete` currently say "checkout notice" narrowly
   (checking the environment-Delete notice line as well).
3. Update `crates/farhelm/src/agent_instructions.rs` (`"Report any checkout notice printed on stderr."` ->
   `"Report any Delete notice printed on stderr."` or equivalent plain wording) and any unit test asserting that exact
   text.
4. Update comments, test comments, and log/panic wording in `crates/farhelm-helm/src/agent_requests.rs`,
   `crates/farhelm-helm/src/notices.rs` (`"late checkout notice task panicked"`), and
   `crates/farhelm/tests/e2e/agent_relay.rs`.
5. Amend `releasing/changelog.d/agent-delete-checkout-notice.md` to cover all agent-Delete cleanup warnings, and remove
   the covered entry from `TODO.md`.

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
   `cargo xtask records path plan/agent-delete-notices-bell-alignment` (`plans/.agents/outside-files.md`, "Files outside
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
  `farhelm-plan-agent-delete-notices-bell-alignment-watchdog.json` in the parent directory of the executing checkout
  (never inside the `plan/agent-delete-notices-bell-alignment` record directory). Watch the executing checkout root,
  `$HOME`, `/tmp`, the parent directory holding the status file, your scratch directory, and any worktree or build
  directory on another filesystem, at a 60-second interval with default alert levels. Follow `scode-build-goal`, "What
  the goal file must contain" (Resource watchdog) for verification, monitoring, `watchdog check` before launches and at
  boundaries, acting on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/agent-delete-notices-bell-alignment/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks
  `plan/agent-delete-notices-bell-alignment/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
  Restructure a PR that needs correcting rather than stacking a fix on top, including across earlier runs' open PRs.
  Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and
  validation"), `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and the
  docstring/comment rules with `scode-voice`. Run targeted tests for `agent_instructions` and `farhelm-helm`
  notices/agent-delete tests through `python3 scripts/record-test-run.py`, plus `cargo fmt`, `cargo clippy`,
  `dprint check`, and `python3 releasing/check-changelog.py format`.
- **Review gate (explicit demand):** Before finishing any PR, run a fresh-context review on **Opus 5.5** at **high**
  effort (natively if available, otherwise via `$scode-harness-shellout`; do not hardcode a launch command) with this
  charter: review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
  against the goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and
  whether a simpler shape would), and idiomatic code for the languages involved; report findings to a named private file
  in your scratch directory, most severe first, with the file, the quoted code, what is wrong, and the smallest fix;
  edit nothing and touch no VCS state. If the PR edits any Rust tests, also supply `.agents/test-authoring.md` verbatim
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
