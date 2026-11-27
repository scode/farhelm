# Plan: Remove the legacy `POST /api/sandbox-sessions` helm endpoint

## Goal

Remove the unused legacy `POST /api/sandbox-sessions` endpoint, its request struct, and the `require_confirmed_size`
bypass flag from the embedded helm, move remaining helm test coverage onto `POST /api/environment-sessions`, and remove
the stale legacy-route and older-helm wording from `SPEC_impl.md` and protocol doc comments.

Acceptance criteria:

- `POST /api/sandbox-sessions`, `create_sandbox_session`, `SandboxCreateRequest`, and the `require_confirmed_size`
  parameter are removed from `crates/farhelm-helm/`.
- `admit_environment_session` is inlined into `create_environment_session` (its sole remaining caller), keeping
  `EnvironmentCreateRequest::confirmed_size` as `Option<RecipeSize>` with `#[serde(default)]` for unconfirmed launches.
- Remaining test coverage in `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs` runs through
  `POST /api/environment-sessions` with `place: TensorlakeAccount` and `confirmed_size` when confirmed, preserving
  stale-size refusal-body and preflight coverage.
- Stale legacy-route and mixed-version older-helm wording is removed from `SPEC_impl.md`,
  `crates/farhelm-proto/src/session_requests.rs`, and `crates/farhelm-helm/src/sessions_tests.rs`.
- A `kind: none` changelog fragment records the internal cleanup per `AGENTS.md`, "Changelog fragments" (if the PR uses
  a required Conventional Commit type).
- The last code PR removes the `Investigate and remove the legacy POST /api/sandbox-sessions endpoint.` entry from
  `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** Verify why `POST /api/sandbox-sessions` is still around and whether anything
  external depends on it; if nothing does, remove the route, `SandboxCreateRequest`, `require_confirmed_size`, and stale
  `SPEC_impl.md` wording, and move remaining test coverage onto `POST /api/environment-sessions`.
- **Verification completed at planning time:** Nothing in the desktop app, CLI, or any production client calls
  `POST /api/sandbox-sessions`; only `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs` hits it. The helm
  runs only embedded inside the desktop app of the exact same build, so no mixed-version older helm or external caller
  exists.
- **Settled maintainer decisions:**
  - Proceed with removing the legacy route and cleaning up the stale wording and tests.
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** `SPEC.md`, `SPEC_impl.md`, `AGENTS.md` ("Conventional Commits", "Changelog
  fragments", "Tests and validation"), and `plans/AGENTS.md`.
- **Planner-proposed choices (from fresh-context planning review):**
  - Inline `admit_environment_session` directly into `create_environment_session` instead of keeping a single-caller
    wrapper.
  - Collapse `Fixture::session_launch_at` and `Fixture::session_launch_request` into one `/api/environment-sessions`
    helper in `tensorlake_add_tests.rs`.
  - Single PR (`plan/remove-legacy-sandbox-sessions-api/01-remove-sandbox-sessions`).

## Implementation outline

Single PR deleting dead route scaffolding and migrating tests:

1. Remove the `POST /api/sandbox-sessions` route mount from `crates/farhelm-helm/src/lib.rs` and the
   `create_sandbox_session` re-export from `crates/farhelm-helm/src/sessions.rs`.
2. In `crates/farhelm-helm/src/sessions/sandbox.rs`, remove `SandboxCreateRequest`, `create_sandbox_session`, and
   `require_confirmed_size`, and inline `admit_environment_session` into `create_environment_session`. Keep
   `EnvironmentCreateRequest::confirmed_size` as `Option<RecipeSize>` with `#[serde(default)]` because unconfirmed
   launches (containers and Tensorlake skip-asking launches) omit it.
3. In `crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs`:
   - Collapse `Fixture::session_launch_at` and `Fixture::session_launch_request` into a single helper posting to
     `/api/environment-sessions`, with `Fixture::session_launch` supplying `place: TensorlakeAccount` and the stored
     recipe's `confirmed_size` when `confirmed == true`.
   - Remove the legacy/modern `if fails / else` branch in
     `recipe_session_task_publishes_success_and_cleans_a_connected_create_failure` so both arms call
     `f.admitted(true).await`.
   - In `recipe_session_consent_refuses_stale_size_before_allocating`, move the refusal-body text assertion
     (`"The sandbox size changed. Review the updated size before launching."`) onto the `/api/environment-sessions`
     `409` response before deleting the legacy-route assertions.
   - Update the two direct `session_launch_request` calls in
     `recipe_session_preflight_never_builds_a_missing_software_snapshot` to include `place` and `confirmed_size` so they
     continue exercising prompt/checkout preflight refusal rather than failing on missing `confirmed_size`.
4. Clean up stale legacy-route and older-helm wording in `SPEC_impl.md` (the environment-session create and
   billing-confirmation paragraphs), `crates/farhelm-proto/src/session_requests.rs` (`EnvironmentCreateBody` and
   `confirmed_size` doc comments), and `crates/farhelm-helm/src/sessions_tests.rs` (the stale older-helm assertion
   message on unconfirmed `EnvironmentCreateBody` serialization).
5. Remove the covered entry from `TODO.md` and add a `kind: none` changelog fragment if required by the PR's
   Conventional Commit type.

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
   `cargo xtask records path plan/remove-legacy-sandbox-sessions-api` (`plans/.agents/outside-files.md`, "Files outside
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
  `farhelm-plan-remove-legacy-sandbox-sessions-api-watchdog.json` in the parent directory of the executing checkout
  (never inside the `plan/remove-legacy-sandbox-sessions-api` record directory). Watch the executing checkout root,
  `$HOME`, `/tmp`, the parent directory holding the status file, your scratch directory, and any worktree or build
  directory on another filesystem, at a 60-second interval with default alert levels. Follow `scode-build-goal`, "What
  the goal file must contain" (Resource watchdog) for verification, monitoring, `watchdog check` before launches and at
  boundaries, acting on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/remove-legacy-sandbox-sessions-api/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks
  `plan/remove-legacy-sandbox-sessions-api/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
  Restructure a PR that needs correcting rather than stacking a fix on top, including across earlier runs' open PRs.
  Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and
  validation"), `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and the
  docstring/comment rules with `scode-voice`. Run targeted `farhelm-helm` tests (covering
  `provisioning::tensorlake_add_tests` and environment session tests), `cargo fmt`, `cargo clippy`, `dprint check`,
  `python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py`.
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
