# Goal: Lock destination in Replace with and record the launcher list Enter rule

Simplify `Replace with` so its sole purpose is replacing the agent or launch settings (harness, command, model, effort,
permissions, wrapper, prompt, title) in place on the source session's host, folder, and repository/checkout — locking
destination fields and deleting the fresh-clone Replace-with path and warning — and record the general `SPEC.md` rule
that keyboard Enter acts on an arrow-highlighted scrollable list item without GPUI scroll-clipping proofs.

**Source TODO entries (both removed in the last code PR):**

- `Require the fresh-clone warning before Replace with can act.` (`## Plan follow-ups`)
- `Ignore Enter on a clipped recent setup.` (`## Plan follow-ups`)

## Acceptance criteria

- `SPEC.md` ("Lifecycle operations" / "Creation") defines `Replace with` as keeping both the source session's **host**
  and its **destination** (folder and GitHub repository/checkout) fixed, while allowing agent/command, model, effort,
  permissions, wrapper, prompt, conversation selector, and title changes. Clone is the action for moving to a different
  folder, repository, or host.
- `SPEC.md` ("One GUI at a time") records the general UI rule: we do not add post-layout GPUI scroll-clipping proofs to
  reject keyboard Enter on an item the user explicitly highlighted with arrow keys in a scrollable list (Enter acts on
  the highlighted item even if scrolled off screen).
- In the desktop app's `Replace with` launcher mode:
  - Both host and destination (folder, Browse, and the `Folder | Managed checkout` toggle) are locked to the source
    session's host and folder.
  - `replacement_checkout_warning` and the Replace-with fresh-clone / checkout-trash / checkout-preview continuation
    paths are deleted (while `replacement_folder_hint` stays because it accurately describes the locked managed
    checkout).
  - Templates that set a different destination are refused via `held_refusal` (matching existing host-lock behavior),
    and recent setups in `Replace with` are refused consistently via `holds(Field::Destination)` so a recent row never
    shows a destination that would be silently dropped.
- Obsolete fresh-clone Replace-with tests and goldens are removed, and tests verify that `Replace with` locks host and
  destination and refuses destination-changing templates and recent setups.
- No scroll-clipping proof machinery is added for `Ignore Enter on a clipped recent setup.`; that `TODO.md` entry is
  resolved by the `SPEC.md` rule and removed.

## Decisions already made

- **Replace with destination lock (maintainer's proposal):** Lock host, folder, and GitHub repo/checkout in
  `Replace with` (leaving agent/command, model, effort, permissions, wrapper, prompt, and title editable), update
  `SPEC.md`, and delete the fresh-clone Replace-with warning and code.
- **General list Enter rule (`Ignore Enter on a clipped recent setup.`):** Add the general `SPEC.md` rule that Enter
  acts on an arrow-highlighted list item even if scrolled (no GPUI scroll-clipping proofs) and remove that `TODO.md`
  entry.
- **Merge both items into one plan.**
- **Review gate:** Single fresh-context Opus 5.5 agent at high effort with the general review charter below.

## Implementation outline and requirement sources

- **User request & decisions:** Both `TODO.md` entries listed above, resolved by the maintainer's two simplification
  decisions above.
- **Binding repository constraints:** `SPEC.md` ("Creation", "Lifecycle operations", "Managed checkouts", "One GUI at a
  time") and `SPEC_impl.md`.
- **Planner-proposed implementation choices (scope-reviewed):**
  - **Reuse `LaunchDraft::holds(Field::Destination)`:** `crates/gui/farhelm-gui/src/launcher/draft.rs:734-740` already
    has `holds(Field::Destination)` wired across `draft.rs` (8 sites), `view.rs`, `view/checkout.rs`, `view/browse.rs`,
    and `view/search.rs`. Changing `Mode::ReplaceWith { .. } => matches!(field, Field::Host | Field::Destination)` locks
    the folder, hides `Folder | Managed checkout`, disables Browse and checkout preview, and blocks destination search
    rows without adding parallel `Mode::ReplaceWith` branches.
  - **Consistent refusal for templates and recent setups:** Extend `templates/apply.rs::held_refusal` (currently
    checking only `Field::Host`) to also check `draft.holds(Field::Destination)` (refusing templates whose destination
    differs and stripping matching ones). Align `launcher/view/recents.rs` with `search.rs:540-544` (which already
    classifies `Row::Recent` under `Field::Destination`) so recent setups are refused cleanly when
    `holds(Field::Destination)` rather than displaying a folder/repo they silently drop.
  - **Delete dead code:** Delete `replacement_checkout_warning` (`launcher/view/checkout.rs:897`), Replace-with
    fresh-clone checkout-preview continuation paths in `view/search/start.rs` / `checkout.rs`, and their obsolete golden
    fixtures/tests. Keep `replacement_folder_hint` (`checkout.rs:854-892`).

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

Right after the checkout check, resolve the plan's record with `cargo xtask records path plan/launcher-visible-consent`
(`plans/.agents/outside-files.md`, "Files outside the repository") and invoke the `agent-resumeable` skill with that
directory's `log.md`. If `log.md` already exists, read it in full and resume where the previous session left off —
cross-checking against GitHub and jj state per `plans/.agents/executing.md`, "Executing one plan" — rather than starting
over. If it does not exist, this is a fresh start.

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

- **Status file:** `farhelm-plan-launcher-visible-consent-watchdog.json` in the parent directory of the executing
  checkout (derived per `scode-build-goal`, "Placing the files", never inside the `plan/launcher-visible-consent` record
  directory).
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
- Name bookmarks `plan/launcher-visible-consent/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never
  merge.
- Restructure existing PRs in the stack (including ones from an earlier run) instead of stacking corrections on top.
- Never edit anything under `plans/`.
- The last code PR removes both `Require the fresh-clone warning before Replace with can act.` and
  `Ignore Enter on a clipped recent setup.` from `TODO.md`.
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

Every PR in the stack exists on GitHub, has passed its Opus 5.5 high review gate, removes both covered `TODO.md` entries
in the last code PR, and satisfies the latest `## Decisions` entry (if any). Delivering the report is the executor's
closing step, not part of this goal.

## Release note

"Replace with" action is now more restrictive and clearly defined. It's sole purpose is to replace the _agent_ (kind of
harness, permissions, model) etc in place in a new fresh conversation but on the same host, same checkout, same repo,
etc. Use Clone when you want to move to a different folder, repo, host, etc. This was the original intent, but was never
made particularly clear or consistent.
