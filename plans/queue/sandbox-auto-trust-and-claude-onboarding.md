# Sandbox auto workspace trust and skipping Claude Code first-run setup

Written against main at c1d85d8ab on 2026-11-24. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This merged plan covers two closely related `TODO.md` entries (bucket "Tensorlake", subsection "v1", "Wave 0 —
No-UX-design fixes"). Verbatim:

> **Always suppress workspace trust prompts in per-session sandboxes where supported.** Workspace trust makes no sense
> for a fresh per-session sandbox; suppress it automatically whenever the agent supports pre-trusting, rather than
> requiring the user to discover per-agent trust switches in Settings → Agents.

> **Skip Claude Code's first-run setup on new Tensorlake hosts.** Decided by the maintainer on 2026-11-07, answering the
> settings-credentials plan's block: Farhelm delivers a selected Claude subscription to a Tensorlake host but writes no
> other Claude configuration, so the first Claude session on a fresh host can stop at Claude Code's own first-run setup
> (its theme chooser and onboarding), which the user goes through once. Claude Code 2.1.318 stopped there in a fresh
> home and also checks a separate onboarding-completion setting. The fix the maintainer deferred: have Farhelm write the
> smallest set of non-secret first-run settings Claude needs, found by an isolated probe, keeping the user's other
> settings and the normal agent home, and check it with a valid subscription login.

Both entries touch `~/.claude.json` seeding during fresh environment setup, so they are built in one clean stack.

### Acceptance criteria

1. **Isolated probe of Claude Code's minimal first-run config:**
   - Run an isolated probe against the installed `claude` CLI in a private scratch home/config directory (never touching
     or mutating the real user home or real `~/.claude.json`), testing both Subscription (`CLAUDE_CODE_OAUTH_TOKEN` or a
     mirrored private copy of an existing login, if available) and API-endpoint (`ANTHROPIC_BASE_URL` +
     `ANTHROPIC_AUTH_TOKEN` against a local loopback stub) startup.
   - Determine the exact minimal set of non-secret keys in `~/.claude.json` required to skip Claude Code's first-run
     theme chooser and onboarding wizard (setting a theme such as `"dark"` only if Claude requires one), and verify that
     pre-trusting `$HOME` in `projects[<canonical_home>].hasTrustDialogAccepted = true` skips Claude's workspace-trust
     prompt both in `$HOME` itself and in a subdirectory under `$HOME` (such as a managed checkout).
   - Record the tested Claude CLI version, exact keys, and probe observations in the working log and final report.
2. **Seed Claude Code's minimal first-run config during recipe host/sandbox setup:**
   - Extend `CredentialDelivery::apply` (`crates/farhelm-helm/src/provisioning/credential_delivery.rs`) with an
     independent step (not gated only to `DeliveryPhase::Setup` and not nested inside `apply_agent_environment()`, so a
     transient setup error retries cleanly on reconnect and lookup errors/Codex reconnects cannot skip it) to seed the
     minimal non-secret onboarding keys into `~/.claude.json` once per recipe-created host/environment (persistent
     Tensorlake hosts, per-session Tensorlake sandboxes, and Podman session containers), across all recipe Claude modes
     (`Subscription`, `API endpoint`, and `None`).
   - Reuse `merge_settings()` (with `shared: true` so `$HOME`'s permissions are preserved and `~/.claude.json` is
     written atomically with mode `0600`) to preserve any existing `~/.claude.json` contents and keys: merge only the
     missing onboarding keys and never overwrite user-chosen values.
   - Record completion with a single `#[serde(default)]` stamp on the host's delivery `Record` so setup is seed-once and
     idempotent, working immediately on already-built snapshots without requiring snapshot rebuilds.
3. **Automatically suppress workspace trust prompts in all per-session disposable environments:**
   - For every host with `is_session_owned` (`HostRow::is_session_owned()` / `HostStatus.is_session_owned` — covering
     both Tensorlake per-session sandboxes and Podman session containers, while leaving persistent `Add host` Tensorlake
     sandboxes and ordinary Local/SSH hosts under Settings → Agents):
     - Resolve `workspace_trust: Some(true)` automatically across all helm launch-composition entry points (`create`,
       fresh managed-checkout create, initial session in a new sandbox/container, `Restart with`, `Replace with`, and
       agent-requested launches targeting a session-owned environment — including `ApprovalAction::LaunchEnvironment` in
       `agent_requests.rs`, which resolves trust before a `HostId` exists), for every built-in harness where
       `LaunchHarness::offers_workspace_trust()` is true (`Claude`, `Codex`, `Muse`, `Pi`, `Antigravity`), respecting
       existing per-selection conflict guards (`workspace_trust_blocked_by_selection`, e.g., custom Antigravity
       `--settings`). Plain `Replace` continues preserving the stored launch.
     - For Claude Code in a session-owned environment, also seed
       `projects[<canonical_home>].hasTrustDialogAccepted = true` in `~/.claude.json` during the same
       `CredentialDelivery::apply` step (merge-preserving, with no supervisor protocol change). Because Claude extends
       trust to subdirectories, this suppresses trust prompts for both `~` (default non-checkout folder) and managed
       checkouts under `~` without weakening `claude/trust.rs`'s `$HOME` protection on ordinary hosts.
     - Keep `selection.workspace_trust` set for Claude on the helm (no per-harness special-casing in shared helm code),
       and update `crates/farhelm-supervisor/src/agent_kind/claude/trust.rs::resolve` read-only: when
       `home.starts_with(&folder)`, inspect the existing config file and return `Ok(None)` if
       `projects[folder].hasTrustDialogAccepted == true` is already recorded; otherwise keep the exact refusal error and
       warning (so ordinary hosts remain protected and a failed sandbox setup seed still surfaces loudly).
4. **Specs, docs, and tests:**
   - Update `SPEC.md` and `SPEC_impl.md` where they currently state that Farhelm leaves Claude's first-run/onboarding
     metadata to Claude and that workspace trust follows only the off-by-default Settings switches in disposable
     environments.
   - Update affected user-facing docs under `website/` (e.g., Tensorlake sandboxes / Claude / Start a session) where
     they say Claude may show first-run setup or describe trust switches for per-session sandboxes, and update any GUI
     helper copy that explicitly warns `"Claude may show its first-run setup on a new host"` (such as the recipe editor
     help text if it says so).
   - Add unit and helm/e2e tests covering seed-once onboarding merge/create, preserving existing `~/.claude.json` keys,
     auto-trust across all 5 supporting harnesses on session-owned hosts (Tensorlake and Podman), non-session-owned
     persistent Tensorlake hosts still following Settings, and `claude` not printing a false trust warning on `~` in a
     session-owned sandbox.
   - Remove both covered `TODO.md` entries in the final PR of the stack.

## Decisions already made

### Maintainer decisions (binding)

- **Merged plan:** Combine "Always suppress workspace trust prompts in per-session sandboxes where supported" and "Skip
  Claude Code's first-run setup on new Tensorlake hosts" into one plan (`sandbox-auto-trust-and-claude-onboarding`).
- **Auto workspace trust scope:** All per-session disposable environments (`is_session_owned`: Tensorlake per-session
  sandboxes and Podman session containers). Persistent `Add host` Tensorlake sandboxes stay under Settings.
- **Claude `~` pre-trust mechanism:** Seed trust for `~` in `~/.claude.json` once during session-owned environment setup
  (no supervisor protocol change).
- **Onboarding seeding location:** During host/sandbox setup (`CredentialDelivery::setup`, seed-once, merge-preserving,
  works immediately on already-built snapshots).
- **Onboarding Claude modes:** All recipe hosts and sandboxes where onboarding keys are absent (`Subscription`,
  `API endpoint`, and `None`).
- **Isolated probe:** Run an isolated local `claude` probe in a private scratch config directory to verify minimal keys
  for both Subscription and API-endpoint startup (using `"dark"` only if Claude requires a theme key).
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** `No release note needed.` (settled verbatim in `## Release note` below).

### Binding repository constraints

- `AGENTS.md`: Never mutate process environment variables in tests; never touch the live Farhelm install; annotate every
  test sleep with `// sleep-ok:` passing `scripts/check-test-sleeps.py`; keep provider decisions exhaustive per
  `crates/farhelm-helm/src/environment/mod.rs` (no `matches!` over providers or provider comparisons in shared code);
  keep harness decisions in the named per-harness homes mapped in `crates/farhelm-supervisor/src/agent_kind/mod.rs`.
- `SPEC.md`: Never weaken `claude/trust.rs`'s refusal to trust `~` or `/` on ordinary hosts.
- `CredentialDelivery` (`crates/farhelm-helm/src/provisioning/credential_delivery.rs`): Must remain atomic (`umask 077`,
  mode `0600`), seed-once via durable `Record` stamps, and preserve existing user keys in `~/.claude.json`.

### Planner-chosen implementation shape

- **Reuse `CredentialDelivery::apply` and `merge_settings()` (`shared: true`) with a `#[serde(default)]` `Record`
  stamp** rather than adding a second provisioning pass or a new supervisor RPC. Putting the step independently in
  `apply()` reuses `CredentialDelivery`'s existing atomic mode-0600 JSON merge, ensures retries on reconnect after a
  transient setup error, and keeps it independent of `apply_agent_environment()` lookup failures.
- **Extend `workspace_trust.rs`'s existing `TrustResolvedLaunch` policy constructors** with an explicit policy input
  (e.g., `session_owned: bool` or a two-variant enum passed to both `normalize_launch_workspace_trust` and
  `configured_launch_workspace_trust`, covering `ApprovalAction::LaunchEnvironment` before a `HostId` exists) so every
  launch route (`sessions.rs`, `agent_requests.rs`) enforces session-owned auto-trust at compile time.
- **Handle already-trusted `$HOME` read-only in `claude/trust.rs::resolve`** instead of special-casing Claude in shared
  helm code or bumping the wire protocol: `resolve` returns `Ok(None)` when `home.starts_with(&folder)` and
  `projects[folder].hasTrustDialogAccepted == true` is already set in `~/.claude.json`, and keeps the exact refusal
  otherwise.

## Outline

### Existing mechanisms reused

- `crates/farhelm-helm/src/provisioning/credential_delivery.rs` (`CredentialDelivery::setup`, `Record`,
  `merge_settings`, `backend.install_bytes`, `credential_delivery_tests.rs`).
- `crates/farhelm-helm/src/workspace_trust.rs` (`TrustResolvedLaunch`, `normalize_launch_workspace_trust`,
  `configured_launch_workspace_trust`).
- `crates/farhelm-helm/src/sessions.rs` (`create_in_session_sandbox`, `create_session_request`,
  `github_checkout_resolution`, restart/replace composition).
- `crates/farhelm-helm/src/agent_requests.rs` (agent-requested environment and session launch composition).
- `crates/farhelm-helm/src/launches.rs` (`with_workspace_trust`, `workspace_trust_blocked_by_selection`).
- `crates/farhelm-proto/src/launch.rs` (`LaunchHarness::offers_workspace_trust`).

### PR stack

1. **`plan/sandbox-auto-trust-and-claude-onboarding/01-claude-onboarding-and-home-trust-seed`** — Seed Claude Code's
   minimal non-secret onboarding config (and, for `is_session_owned` environments, home-directory workspace trust) once
   in `~/.claude.json` during `CredentialDelivery::setup`:
   - Run the isolated local `claude` probe first and record findings in the working log.
   - Add a seed-once `Record` stamp and merge-preserving atomic `~/.claude.json` helper in
     `crates/farhelm-helm/src/provisioning/credential_delivery.rs` covering all recipe hosts/sandboxes (`Subscription`,
     `API endpoint`, and `None`), adding `projects[<home>].hasTrustDialogAccepted = true` when `row.is_session_owned()`.
   - Update recipe editor helper copy if it still warns that Claude may show first-run setup on a new host, plus unit
     tests in `credential_delivery_tests.rs`.
2. **`plan/sandbox-auto-trust-and-claude-onboarding/02-session-owned-auto-workspace-trust`** — Automatically resolve
   workspace trust in `workspace_trust.rs` for all `is_session_owned` hosts across all launch routes and supported
   harnesses:
   - Pass destination `is_session_owned` into `workspace_trust.rs` policy constructors across `sessions.rs` and
     `agent_requests.rs` so all 5 `offers_workspace_trust()` harnesses resolve trusted automatically on session-owned
     environments (while persistent `Add host` sandboxes and ordinary hosts continue honoring Settings).
   - Ensure Claude launches in `~` on session-owned environments do not emit a false
     `"Farhelm did not mark this folder trusted"` warning (since PR 1 already seeded home trust in `~/.claude.json`).
   - Update `SPEC.md`, `SPEC_impl.md`, affected docs under `website/`, add helm/supervisor unit and e2e tests, add the
     changelog fragment(s), and remove both covered `TODO.md` entries.

## Fallbacks

- If the isolated local `claude` probe cannot test a real Subscription login because no Claude subscription token or
  login is available on the executing host, verify against a local loopback API-endpoint stub (and against Claude's
  documented/observed onboarding schema), record the exact limitation in the working log and final report, and do not
  block unless the minimal keys themselves cannot be established.
- If Claude Code 2.1.x requires a non-obvious extra key or refuses to skip onboarding even with `hasCompletedOnboarding`
  and `theme` in an isolated probe, block per `plans/.agents/executing.md` (step 10) with the exact observed screen and
  keys tested.
- Scope-reassessment triggers (examples, not a blacklist): bumping the helm–supervisor wire protocol, modifying
  `claude/trust.rs`'s `$HOME` refusal for ordinary hosts, or touching `build_script.rs` / snapshot builds.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-sandbox-auto-trust-and-claude-onboarding-log.md` in the parent directory of the checkout you run in,
derived as that section says, and the watchdog's status file is
`farhelm-plan-sandbox-auto-trust-and-claude-onboarding-watchdog.json` beside it. Resolve both to absolute paths before
you start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`, "Agent
scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- Targeted Rust tests per PR (through `record-test-run.py` with pinned nextest on `PATH`):
  - `cargo nextest run -p farhelm-helm --lib -E 'test(credential_delivery) | test(workspace_trust) | test(sessions_tests) | test(agent_requests)'`
  - `cargo nextest run -p farhelm-supervisor --lib -E 'test(claude::trust) | test(agent_kind)'` (if supervisor launch
    trust handling is touched)
  - `cargo nextest run -p farhelm-gui --lib -E 'test(recipes)'` (if GUI recipe help text is touched)
- Formatting, lints, and checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
  - `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` (if Rust tests change)
  - `dprint check` on changed Markdown files
  - `python3 releasing/check-changelog.py format`
  - `python3 scripts/check-agents-md.py` (if headings or instruction files change)

Apply `.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live
Farhelm install on this machine (root `AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes both `TODO.md` entries this plan covers ("Always suppress workspace trust prompts in
  per-session sandboxes where supported." and "Skip Claude Code's first-run setup on new Tensorlake hosts."), as root
  `AGENTS.md`, "TODO.md and FLAKES.md", requires. Any deferred items added to `TODO.md` go under `## Plan follow-ups`.
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
alternatives considered: in particular the isolated Claude probe findings, how `claude/trust.rs` avoids a false warning
for `~` in session-owned sandboxes, and every review finding you decided not to follow. The maintainer will ask for
these later.

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
DECISION, or it is a block. Things the maintainer should know and possible follow-ups go into the report as statements.

### Done criterion

The goal is achieved when every PR in the linear stack exists on GitHub as a draft PR and has passed its review gate,
and the latest `## Decisions` entry, if any, is satisfied. Delivering the report is your closing step per
`plans/.agents/executing.md`, "Executing one plan" (step 11), not part of the goal.

## Release note

No release note needed.
