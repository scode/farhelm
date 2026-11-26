# Goal: Bind leftover sandbox consent to the saved Tensorlake key

In Settings -> Tensorlake, immediately close any open leftover-sandbox Delete confirmation and clear stale discovered
rows, cleanups, and errors whenever the saved Tensorlake API key is cleared or replaced (from either
`Settings -> Tensorlake` or `Settings -> Credentials`), so Delete can never act on stale rows from a previous key.

**Source TODO entry (removed in the last code PR):** `Bind leftover sandbox consent to the saved Tensorlake key.` under
`## Plan follow-ups` in `TODO.md`.

## Acceptance criteria

- Replacing or clearing the Tensorlake API key from either key form (`Settings -> Tensorlake` or
  `Settings -> Credentials`) immediately retires any open leftover-sandbox Delete confirmation (`question`, `asked`) and
  clears `last_discovered`, `cleanups`, `delete_failure`, `discovery_error`, and `checked_at` (without clearing
  `deleting` while an orphan Delete is still in flight), requiring a fresh discovery list before offering Delete again.
- When a key write returns `Outcome::Done` with an empty or undecodable body, `TensorlakeAccount::key_saved` clears
  `self.settings = None`, starts `Self::read(cx)`, and calls `cx.notify()` immediately (leaving `acknowledged()`
  unchanged for `create_without_asking`). In `Sandboxes::follow_account`, when `account.key_saves()` changes while
  `account.settings().is_none()`, immediately retire the prompt and clear stale rows/errors as above, bump `self.round`,
  reset **`self.key = None`** (never recording `Some((old_bool, n + 1))`, which would short-circuit the equality check
  when the follow-up read answers and skip starting `read(cx)`), reset **`self.checking = false`**, and call
  **`cx.notify()`** before returning — so the retired prompt repaints immediately, stale `read()`/`delete()` replies see
  `list.key != key`, and the subsequent account read starts a fresh discovery read when a key is saved.
- `Sandboxes::delete()` captures `let key = self.key` before sending: on settlement it always resets `deleting = false`,
  and updates `last_discovered` / `cleanups` / `delete_failure` or triggers `read(cx)` only when `list.key == key`, so a
  late reply from an earlier key cannot pollute a new key's list or resurrect the hidden Sandboxes section after Clear.
- On a definite key-write refusal, the key form keeps its draft and error; once the fresh list returns after a key
  change, any unfinished cleanup reported by the helm is shown.
- Update `SPEC_impl.md:7396-7403` to note that both Clear and key replacement retire any open leftover-sandbox Delete
  confirmation and clear cached discovery state (including across empty or undecodable replies).
- GUI tests cover both key forms (`Place::Tensorlake` and `Place::Credentials`), an empty/undecodable `Done` reply
  (verifying immediate prompt retirement AND that the follow-up read triggers fresh discovery), stale discovery
  rejection, and an in-flight orphan Delete racing a key change; light and dark captures of the replacement view are
  checked if visuals change.

## Decisions already made

- **Drop slow-helm / in-flight latency machinery:** Under the maintainer's low-latency helm rule for Settings (and
  because the helm's `delete_orphan` re-reads and re-validates the current key on disk at deletion time), do not add a
  `key_writes_in_flight` counter, held-write tests, or 5xx generation bumps. Keep the fix to the non-latency
  `Sandboxes::follow_account` / `key_saved` / `delete` invariants above.
- **Independent of `settings-wait-while-saving`:** Runs cleanly before or after `settings-wait-while-saving.md` with no
  queue dependency.
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter below.

## Implementation outline and requirement sources

- **User request & decisions:** `TODO.md` ("Bind leftover sandbox consent to the saved Tensorlake key.") narrowed by the
  settled low-latency helm simplification decision above.
- **Binding repository constraints:** `SPEC.md` ("Keyboard focus", "Settings", "Sandbox sessions") and `SPEC_impl.md`
  (`7396-7403`).
- **Planner-proposed implementation choices (two rounds of dual Neon + Opus reviews, 1 PR):**
  - Reuse `TensorlakeAccount`'s existing `key_saves` counter (`crates/gui/farhelm-gui/src/settings/tensorlake_key.rs`)
    and `Sandboxes::follow_account` / `round` gating (`crates/gui/farhelm-gui/src/settings/tensorlake_sandboxes.rs`).
  - In `TensorlakeAccount::key_saved`, when `body` is `None` or fails to decode, set `self.settings = None`, call
    `Self::read(cx)`, and call `cx.notify()` (preserving `acknowledged()` for `create_without_asking` so
    `a_done_save_whose_answer_does_not_decode_reads_the_account_again` stays intact).
  - In `Sandboxes::follow_account`, handle a `key_saves` change before requiring `account.settings().is_some()`:
    - When `account.settings()` is `None` and `key_saves` changed, clear `question`, `asked`, `delete_failure`,
      `discovery_error`, `last_discovered`, `checked_at`, and `cleanups` (leaving `deleting` untouched), bump
      `self.round`, set `self.key = None`, set `self.checking = false`, and call `cx.notify()` before returning.
    - When `account.settings()` is `Some(settings)` and `self.key != Some((settings.api_key_set, key_saves))`, retire
      and clear the same fields, bump `self.round`, update `self.key`, and start a fresh `self.read(cx)` when
      `settings.api_key_set` is true.
  - In `Sandboxes::delete()`, capture `let key = self.key;` and guard the reply's state updates and follow-up `read(cx)`
    on `list.key == key` (always resetting `list.deleting = false`).
  - Update `SPEC_impl.md:7396-7403`, add focused tests, and remove the `TODO.md` entry.

## Scope reassessment

Before implementing a substantial departure from the outline, run a fresh-context review at the executing session's own
model and effort with this charter:

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

Supply the request, decisions, outline, current diff, and proposed departure. Repeated corrective reviews of the same
component also require reassessing its necessity before another repair round. Give same-model sub-agents bounded units
within the outline; unapproved new mechanisms are outside their task.

## The checkout

This goal runs inside the checkout the executing session runs in, identified by GitHub repository `scode/farhelm` (the
nearest Git worktree or jj workspace containing the working directory; never identified by path). Confirm first that the
nearest checkout has a remote pointing at `scode/farhelm`; if not, stop before the resume protocol, write nothing, and
report the mismatch. Every project file path in this plan is repo-relative.

## Resume protocol

Right after the checkout check, resolve the plan's record with
`cargo xtask records path plan/tensorlake-sandbox-key-consent` (`plans/.agents/outside-files.md`, "Files outside the
repository") and invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already exists, read it
in full and resume where the previous session left off — cross-checking against GitHub and jj state per
`plans/.agents/executing.md`, "Executing one plan" — rather than starting over. If it does not exist, this is a fresh
start.

## Execution mode

Explicit **no-workhorse mode**: do not send any unit of your own decomposition to a weaker model or lower effort than
your own, and do not invoke, load, or follow `scode-galaxy-brain` or `scode-model-routing` at any point (if
`scode-galaxy-brain` is active when the run starts, stop it). Same-model sub-agents at your own model and effort are
allowed whenever they finish work sooner without costing quality, using separate worktrees or jj workspaces if
concurrent writers might conflict. The review gate keeps the explicit Opus 5.5 high demand below, scope reassessment
runs at your own model and effort, and the required sub-agents listed in `plans/AGENTS.md`, "Sub-agents this file
requires" (resume check, cold reads of Blocked questions and reports), are exempt from any no-delegation restriction.
Launch any sub-agent whose model is unavailable natively through `scode-harness-shellout`.

## Resource watchdog

Right after the resume protocol and before any delegation, start the `scripts/watchdog` Deno script from the installed
`scode-build-goal` skill directory (resolved with `realpath` at run time) to monitor memory and disk for the rest of the
run:

- **Status file:** `farhelm-plan-tensorlake-sandbox-key-consent-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files", never inside the `plan/tensorlake-sandbox-key-consent`
  record directory).
- **Watched paths (60s interval, default alert thresholds):** the executing checkout root, `$HOME`, `/tmp`, the parent
  directory holding the status file, the session's scratch directory, and any temporary worktree or build directory
  created on another filesystem.
- **Verification & lifecycle:** Verify the delivery mechanism on throwaway status files in scratch space
  (`--interval 5`, `--synthetic-alert`, killed pid, `--stall-after 2`) before arming the real monitors (Claude Code dual
  Monitors; Codex `wake-on-exit`; plain background fallback with `watchdog check` before launches and at boundaries if
  neither is available). Run `watchdog check --status <file>` before every new workload launch and at natural
  boundaries. Act immediately on `ALERT`/`WORSE`/`STALE`/`ERROR` by pausing new launches and freeing run-owned build
  artifacts; stop owned monitors at close.

## Stack, PR discipline, and repository conventions

- Build a linear stack of bite-sized draft PRs on top of `main@origin` (or this plan's own open PRs when resuming, never
  another plan's PRs) using the `jjstack` skill, set up per `plans/.agents/executing.md`, "Executing one plan".
- Name bookmarks `plan/tensorlake-sandbox-key-consent/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never
  merge.
- Restructure existing PRs in the stack (including ones from an earlier run) instead of stacking corrections on top.
- Never edit anything under `plans/`.
- The last code PR removes `Bind leftover sandbox consent to the saved Tensorlake key.` from `TODO.md`.
- Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and validation") and
  `.agents/validation.md`, `.agents/test-evidence.md`, and `.agents/test-authoring.md` (supply
  `.agents/test-authoring.md` verbatim to the reviewer when tests or fixtures change).

## Review gate

Before finishing any PR in the stack, run a review by a single fresh-context **Opus 5.5** agent at **high** effort
(explicit demand for model and effort; launch natively or via `scode-harness-shellout` without hardcoding a CLI command
here). Give the reviewer the repo root, the commit range or bookmark to review, the private findings file path in
scratch space, and `.agents/test-authoring.md` verbatim if tests changed, with this charter:

> Review the PR's changes for general correctness (bugs, unhandled cases, broken invariants, wrong behavior against the
> goal's acceptance criteria), design (whether the shape of the change fits the codebase and the goal, and whether a
> simpler shape would), and idiomatic code for the languages involved; report findings to the named file, most severe
> first, with the file, the quoted code, what is wrong, and the smallest fix; edit nothing and touch no VCS state.

Address what it finds before moving on.

## Decision logging and unattended fallback

Log consequential design choices in the working log as `DECISION` with alternatives considered. Resolve routine
implementation forks within agreed scope and log them; remove unnecessary planner-invented machinery when agreed
behavior and repository constraints still hold.

Never ask the maintainer anything in conversation — not a direct question, not a harness question tool prompt, not an
optional offer or closing question (`plans/AGENTS.md`, "Unattended means no questions"). Every fork is either decided
within scope and logged as `DECISION`, or — when a material scope expansion, weakened guarantee, or maintainer-only
product choice cannot be resolved from the plan, its `## Decisions`, and the specs — blocked per
`plans/.agents/executing.md`, "Executing one plan", step 10. Things the maintainer should know and optional follow-ups
go into the report.

## Done criterion

Every PR in the stack exists on GitHub, has passed its Opus 5.5 high review gate, removes the covered `TODO.md` entry in
the last code PR, and satisfies the latest `## Decisions` entry (if any). Delivering the report is the executor's
closing step, not part of this goal.

## Release note

No release note needed.
