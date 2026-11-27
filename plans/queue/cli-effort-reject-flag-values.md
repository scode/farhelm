# Plan: Reject flag-like values for CLI `--effort` and `--model`

## Goal

Stop the `farhelm` CLI from consuming a following flag as the literal value of `--effort` or `--model` when the caller
omits the option's value (for example `--effort --yolo` or `--model --yolo`).

Acceptance criteria:

- `LaunchFlags` in `crates/farhelm/src/main.rs` drops `allow_hyphen_values = true` on both `--effort` and `--model`, so
  separate hyphen-leading tokens such as `--effort --yolo`, `--effort -fast`, `--model --yolo`, and `--model -fast` fail
  at CLI parse time.
- Explicit `=` forms (`--effort=-fast`, `--model=-fast`) continue to parse for custom agents that allow leading-hyphen
  identifiers.
- `SPEC_impl.md` reflects the updated CLI parsing rule for `--effort` and `--model`.
- CLI parse tests cover rejecting `--effort --yolo`, `--effort -fast`, `--model --yolo`, and `--model -fast`, and
  accepting `--effort=-fast` and `--model=-fast`.
- The unreleased changelog fragment `releasing/changelog.d/custom-effort-leading-hyphen.md` (which advertised the
  reverted `--effort -fast` behavior from PR #4954) is updated or removed per `AGENTS.md`, "Changelog fragments".
- The last code PR removes the `Reject flag-like values for CLI --effort.` entry from `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** PR #4954 added `allow_hyphen_values = true` to `--effort` on `LaunchFlags` so
  `--effort -fast` worked without `=`. The side effect is that `--effort --yolo` consumes `--yolo` as the effort string
  instead of failing to parse; custom agents then launch with `--yolo` as their effort. Reject flag-like values for
  `--effort`, update `SPEC_impl.md` and CLI parse tests, and check whether `--model` should follow the same rule.
- **Settled maintainer decisions:**
  - Drop `allow_hyphen_values = true` on `--effort` so separate hyphen-leading tokens fail at clap parse time while
    `--effort=-fast` still works.
  - Apply the exact same rule to `--model` (drop `allow_hyphen_values = true` on `--model` too, fixing the identical
    `--model --yolo` custom-agent bug).
  - Do not change the custom-agent wrapper contract (`validate_model_or_effort` in
    `crates/farhelm-proto/src/custom_agents.rs`) as part of this plan.
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** `SPEC_impl.md` ("CLI" / `LaunchFlags` parsing rules), `AGENTS.md` ("Conventional
  Commits", "Changelog fragments", "Tests and validation"), and `plans/AGENTS.md`.
- **Planner-proposed choices:**
  - Single PR (`plan/cli-effort-reject-flag-values/01-reject-flag-values`).
  - Leave `AgentCmd`'s creating-verbs doc comment in `crates/farhelm/src/main.rs` minimal (it does not list `--model` or
    `--effort`, though a brief parenthetical clarifying they are excluded from the `allow_hyphen_values` generalization
    is fine).

## Implementation outline

This is a single small PR with no new mechanisms; it relies directly on clap's default option parsing:

1. In `crates/farhelm/src/main.rs`, remove `allow_hyphen_values = true` from `model` and `effort` on `LaunchFlags`. Keep
   `parse_effort` intact.
2. Update `SPEC_impl.md` where it describes free-text value options (`--model`, `--effort`, `--cwd`) accepting
   `-`-prefixed values as separate arguments, restoring the `=` form note for leading-hyphen custom model/effort values.
3. Replace `custom_create_accepts_a_separate_hyphen_leading_effort` in `crates/farhelm/src/main.rs` and add parse tests
   verifying that `--effort --yolo`, `--effort -fast`, `--model --yolo`, and `--model -fast` fail at parse time while
   `--effort=-fast` and `--model=-fast` parse cleanly.
4. Handle `releasing/changelog.d/custom-effort-leading-hyphen.md`: PR #4954 is unreleased, so do not leave a fragment
   advertising the reverted `--effort -fast` behavior (convert it to `kind: none` explaining the reverted unreleased
   behavior, or remove it and leave a `kind: none` fragment if needed to cover the `fix:` commit per `AGENTS.md`,
   "Changelog fragments").
5. Remove the covered entry from `TODO.md`.

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
   `cargo xtask records path plan/cli-effort-reject-flag-values` (`plans/.agents/outside-files.md`, "Files outside the
   repository"). Immediately invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already
   exists, read all of it and resume from where the previous session left off, cross-checking against GitHub and VCS
   state (`plans/.agents/executing.md`, "Executing one plan", step 7); otherwise start fresh.

## Execution mode and resource watchdog

- **No-workhorse mode:** This is an explicit no-workhorse choice. Never send any unit of your own decomposition to a
  weaker model or lower effort than your own, and never invoke, load, or follow `scode-galaxy-brain` or
  `scode-model-routing` at any point (if `scode-galaxy-brain` is already active, stop it). Same-model, same-effort
  sub-agents are allowed at your judgment when likely to finish sooner without costing quality (using separate worktrees
  or jj workspaces if concurrent writers would conflict, while you retain ownership of `jjstack` and every PR). The
  sub-agents required by `plans/AGENTS.md`, "Sub-agents this file requires" (resume check, cold reads of Blocked
  questions and reports), the per-PR review gate below, and any scope reassessment review are exempt from any
  no-delegation rule. Launch sub-agents natively where supported, or through `$scode-harness-shellout` when the demanded
  model is unavailable natively.
- **Resource watchdog:** Right after the resume protocol and before any delegation, start the `scode-build-goal` skill's
  `scripts/watchdog` (resolved with `realpath` at run time) using
  `farhelm-plan-cli-effort-reject-flag-values-watchdog.json` in the parent directory of the executing checkout (never
  inside the `plan/cli-effort-reject-flag-values` record directory). Watch the executing checkout root, `$HOME`, `/tmp`,
  the parent directory holding the status file, your scratch directory, and any worktree or build directory on another
  filesystem, at a 60-second interval with default alert levels. Follow `scode-build-goal`, "What the goal file must
  contain" (Resource watchdog) for verification, monitoring, `watchdog check` before launches and at boundaries, acting
  on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/cli-effort-reject-flag-values/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks
  `plan/cli-effort-reject-flag-values/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
  Restructure a PR that needs correcting rather than stacking a fix on top, including across earlier runs' open PRs.
  Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and
  validation"), `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and the
  docstring/comment rules with `scode-voice`. Run targeted validation for `crates/farhelm` CLI parse tests, `cargo fmt`,
  `cargo clippy`, `dprint check`, `python3 releasing/check-changelog.py format`, and
  `python -B scripts/check-test-sleeps.py` if Rust tests changed.
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
