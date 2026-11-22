# Move Codex recipe endpoint credentials to launch-time delivery

Written against main at 8007a789 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers two merged TODO.md entries (bucket "Plan decisions to revisit" and bucket "Near term", subsection "Plan
follow-up"). Verbatim:

> **Revisit deferring the Codex credential migration.** On 2026-11-22 the landing monitor answered
> `recipe-credentials-in-launch-env`'s second block on the maintainer's behalf: the plan moves Claude's recipe
> credentials into the launch environment now and leaves Codex's delivery as it is, settings writes included, with the
> Codex migration recorded as a Plan follow-up. The executor found that Codex 0.171.0 launch overrides cannot keep a
> leftover settings bearer away from a tokenless endpoint: leaving the token out keeps the stale bearer, and an empty
> one sends an empty Authorization header. The alternative was to widen the plan to find and verify a way to isolate
> Codex's launch configuration that keeps saved conversations, regular settings and tokenless endpoints working.

> **Move Codex recipe endpoint credentials out of its settings.** Farhelm can overwrite a concurrent save to
> `~/.codex/config.toml` when it delivers a recipe endpoint. Keep the `farhelm` provider ID so existing sessions can
> restart and the launcher can open saved conversations by ID. In an isolated Codex 0.171.0 loopback probe, a launch
> override that omitted the token field sent the stale settings bearer to the new endpoint (recorder
> `c235a277-a9fb-497d-a323-ebb0538a8976`). Explicitly clearing the field sent an empty Authorization header (recorder
> `59601200-f69b-4179-99a6-a94f4a685ec5`). Find an override or compatible isolation mechanism that preserves saved
> conversations, user settings and requests to tokenless endpoints without an Authorization header.

In product terms: stop writing Codex recipe API endpoint settings into `~/.codex/config.toml` on environment hosts
(Tensorlake sandboxes and Podman containers). Mirror Claude's migration from `recipe-credentials-in-launch-env`: run a
one-time cleanup on hosts where an earlier Farhelm stamped `~/.codex/config.toml`, deliver Codex endpoint credentials in
Farhelm's private `<state_dir>/agent-credentials` file, and inject Codex's `-c` provider overrides (with any bearer
token supplied via environment variable `env_key`, never on argv) when Farhelm launches or resumes Codex sessions. Keep
the `farhelm` provider ID so existing sessions can restart and saved conversations can still be opened by ID, and keep
Codex Subscription delivery (`~/.codex/auth.json`) unchanged.

### Where it stands (from planning research at main 8007a789; line numbers approximate)

- **Supervisor private credentials file (`crates/farhelm-supervisor/src/recipe_credentials.rs`):**
  - `RecipeCredentials { env: BTreeMap<String, String> }` has `#[serde(deny_unknown_fields)]`, lives at
    `<state_dir>/agent-credentials` (mode `0600`), and is read/injected at spawn in
    `service/core/host_launch.rs:465-469` and `service/core/host_tabs.rs:611-619`.
  - Extend `RecipeCredentials` with a non-secret optional Codex endpoint descriptor (e.g.
    `codex_endpoint: Option<CodexEndpoint>` holding `base_url: String` and `has_token: bool`) while storing the bearer
    token itself in `RecipeCredentials.env` under a single fixed internal env var name (e.g.
    `pub const CODEX_ENDPOINT_TOKEN_ENV: &str = "FARHELM_CODEX_ENDPOINT_TOKEN"`), validated in
    `RecipeCredentials::validate()` so `has_token` matches presence of that key in `env`.
  - **Secret boundary (`SPEC_impl.md:6015-6016`):** Never place the bearer token in `FarhelmArgs.env` (which is
    serialized to `LaunchSpec` on disk in `prepare_launch`, `service/core.rs:14580, 14668`). Pass only the non-secret
    `Option<&CodexEndpoint>` metadata into `prepare_launch` / `HookPolicy` / `CodexIntegration::farhelm_args`; the token
    reaches `ChildSpec.env` solely through `credentials.inject(&mut env)` (`host_launch.rs:530`) over `terminal-host`
    stdin.
  - Bump `PROTOCOL_VERSION` in `crates/farhelm-proto/src/lib.rs` (currently `86` -> `87`) so the helm refuses older
    supervisors before writing an extended `agent-credentials` file they would reject on spawn.
- **Supervisor Codex launch args (`crates/farhelm-supervisor/src/agent_kind/mod.rs`):**
  - `CodexIntegration::farhelm_args` (`mod.rs:1368-1374`) injects `--no-daemon` unconditionally on Start and Resume
    (even when `FARHELM_AGENT_HOOKS=none`) plus `-c` hook overrides when hooks are enabled.
  - Reuse the existing `toml_basic_string` helper (`agent_kind/mod.rs:1498`, round-trip tested against `toml` in
    dev-dependencies; `farhelm-supervisor` deliberately has no runtime TOML writer crate) to format `base_url` and
    `env_key` in `-c model_provider=farhelm` and
    `-c model_providers.farhelm={name="Farhelm",base_url=...,wire_api="responses",requires_openai_auth=false[,env_key=...]}`
    (independent of `FARHELM_AGENT_HOOKS`, on both Start and Resume). For tokenless endpoints (`has_token == false`),
    omit `env_key` and `experimental_bearer_token`, so clean and migrated hosts send no `Authorization` header.
- **Helm credential composition & delivery (`crates/farhelm-helm/src/provisioning/agent_endpoints.rs`,
  `credential_delivery.rs`):**
  - Keep `clear_codex()` (`agent_endpoints.rs:112-137`) removing only `model_provider = "farhelm"` and
    `model_providers.farhelm.experimental_bearer_token` (preserving the table, comments, and any user-authored keys per
    `SPEC.md:533-534`; earlier Farhelm never wrote `env_key`, `http_headers`, or `env_http_headers`). Remove the
    now-dead `agent_endpoints::codex()` helper (`agent_endpoints.rs:76-106`) so clippy stays clean.
  - In `credential_delivery.rs` (`Record` at lines 123-136, `apply_claude()` at lines 772-879, Codex slot at lines
    635-651, `push_one` at lines 558-566):
    - Run Claude's and Codex's one-time legacy cleanups independently first (separate `claude` / `codex` legacy stamps
      and `claude_environment` / `codex_environment` launch stamps in `Record`).
    - Assemble a single `RecipeCredentials` snapshot containing only the agents whose legacy cleanup is `Cleared`, and
      atomically install or unlink `<state_dir>/agent-credentials` once. If one agent's legacy cleanup fails, withhold
      only that agent's credentials from the snapshot while still publishing the other's (`SPEC_impl.md:6021-6023`).
    - Track Claude and Codex pending-cleanup warnings separately in `apply()` and `push_one()` (adding
      `uses_codex_endpoint(name)` alongside `uses_claude(name)`) so replacing an unrelated credential (such as GitHub or
      Codex Subscription) never misreports another agent's pending cleanup.
- **GUI Settings help & docs:**
  - `crates/gui/farhelm-gui/src/settings/recipes/editor/credentials.rs:126-128`: update Codex's help text to note that
    endpoint credentials are supplied when Farhelm launches Codex sessions and to restart a running session to pick up a
    replacement (matching Claude's help text at lines 109-111, while retaining the Codex Subscription `auth.json`
    sentences); refresh and publish affected Settings golden screenshots.
  - Update `SPEC.md:525-545`, `SPEC_impl.md:5992-6025` ("Named credential catalog") and `6558-6581`, and
    `website/src/content/docs/docs/using/tensorlake-sandboxes.md` (lines 111-150).

### Acceptance criteria

1. **No ongoing `~/.codex/config.toml` writes:** Delivering, updating, or removing a Codex recipe API endpoint never
   modifies `~/.codex/config.toml` on fresh or already-migrated hosts. Codex Subscription delivery
   (`~/.codex/auth.json`) is unchanged.
2. **One-time stamped legacy cleanup:** On hosts whose delivery record carries a legacy `codex` stamp from an earlier
   Farhelm, cleanup removes `model_provider = "farhelm"` and `model_providers.farhelm.experimental_bearer_token` once
   before publishing launch-time Codex credentials (withholding only Codex launch credentials and surfacing a scoped
   non-fatal warning if cleanup fails, without blocking Claude's delivery or misattributing warnings in `push_one`).
3. **Launch-time Codex endpoint injection:**
   - The supervisor reads Codex endpoint configuration from `<state_dir>/agent-credentials` and appends Codex `-c`
     overrides (`model_provider=farhelm` and `model_providers.farhelm={...}`, escaped via `toml_basic_string`) on Start
     and Resume (including when `FARHELM_AGENT_HOOKS=none`).
   - Any bearer token lives only in `RecipeCredentials.env` and reaches the child via `ChildSpec.env` (`env_key`), never
     in argv and never in `LaunchSpec` on disk.
   - A tokenless endpoint omits `env_key` and `experimental_bearer_token`, sending no `Authorization` header on clean or
     migrated hosts.
   - The provider ID remains `farhelm`, preserving Restart and saved conversation resume by ID.
   - `PROTOCOL_VERSION` is bumped so older supervisors are refused before receiving the extended `agent-credentials`
     format.
4. **Tests, Settings copy, goldens, specs, and docs:**
   - Unit tests cover `recipe_credentials`, `CodexIntegration::farhelm_args` (including special characters in
     `base_url`), `clear_codex`, and `credential_delivery` (including Claude + Codex coexisting in one
     `agent-credentials` file, independent legacy-cleanup failure, `push_one` warning scoping, token rotation, and
     tokenless endpoints).
   - E2E tests (`host_creates.rs` / `podman_environments.rs`) verify delivery without touching `~/.codex/config.toml`
     and without placing the token in `LaunchSpec` or argv.
   - Settings → Recipes Codex help copy is updated and affected golden screenshots are refreshed and published.
   - `SPEC.md`, `SPEC_impl.md` (both catalog and delivery sections), and `tensorlake-sandboxes.md` are updated; both
     TODO.md entries are removed in the last PR.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Merge both Codex credential TODO entries ("Revisit deferring the Codex credential migration" and "Move Codex recipe
  endpoint credentials out of its settings") into this single plan, removing both TODO entries when it lands.
- Use **one-time cleanup of Farhelm-stamped `~/.codex/config.toml` + per-launch `-c` overrides with the token in an
  environment variable (`env_key`)**, matching Claude's trust boundary (accepting the two documented caveats: raw
  `codex` typed manually in a shell tab does not receive the `-c` override, and an unstamped image-baked `config.toml`
  bearer is outside Farhelm's cleanup boundary per `SPEC.md:536-538`).
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `AGENTS.md`, "Code rules": keep Codex launch injection inside `CodexIntegration` (`agent_kind/mod.rs`).
- `SPEC.md` lines 525-545 and `SPEC_impl.md` lines 5992-6025, 6558-6581.
- `website/AGENTS.md` for `tensorlake-sandboxes.md` (unattended run: no preview server).
- `.agents/validation.md` for GUI goldens (`check-gui-goldens.py` + `gui-goldens.py publish`).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "The live install is
  off-limits", and "Instructions by path".

**Planner proposals:**

- Reuse `<state_dir>/agent-credentials` (extending `RecipeCredentials` behind a `PROTOCOL_VERSION` bump) rather than
  creating a second per-host credentials file, keeping Claude and Codex endpoint updates atomic.
- Reuse `agent_kind/mod.rs::toml_basic_string` for TOML string escaping in Codex `-c` overrides.
- Slice into 2 clean PRs: (1) protocol bump + supervisor `recipe_credentials` & Codex launch injection + helm
  `credential_delivery` migration + Settings Codex help copy & goldens + tests, (2) purely `docs:` PR updating
  `SPEC.md`, `SPEC_impl.md`, `tensorlake-sandboxes.md`, and removing both TODO entries.

## Outline

1. `plan/codex-recipe-credentials-at-launch/01-launch-time-codex-credentials`: bump `PROTOCOL_VERSION`, extend
   `RecipeCredentials` with non-secret `CodexEndpoint` (`base_url` + `has_token`) and keep the bearer token in
   `RecipeCredentials.env` (`CODEX_ENDPOINT_TOKEN_ENV`), inject Codex `-c` overrides via `toml_basic_string` in
   `CodexIntegration::farhelm_args` on Start and Resume (independent of hooks), switch helm `credential_delivery.rs`
   Codex endpoint delivery to one-time stamped cleanup + atomic `agent-credentials` write alongside Claude (removing
   now-dead `agent_endpoints::codex()` and scoping `push_one` warnings per agent), update Settings → Recipes Codex help
   copy in `credentials.rs` (retaining Subscription `auth.json` text) and refresh/publish affected Settings goldens, and
   add unit/e2e coverage. `fix: supply Codex recipe endpoint credentials at launch` (with `kind: fixed` changelog
   fragment).
2. `plan/codex-recipe-credentials-at-launch/02-docs-and-todo`: update `SPEC.md`, `SPEC_impl.md` (both "Named credential
   catalog" and delivery sections), and `website/src/content/docs/docs/using/tensorlake-sandboxes.md`, and remove both
   TODO.md entries ("Revisit deferring the Codex credential migration" and "Move Codex recipe endpoint credentials out
   of its settings"). `docs: document launch-time Codex recipe credentials`.

## Fallbacks

- If `PROTOCOL_VERSION` advances on `main@origin` before this lands, move past main's number on rebase per
  `.agents/careful-rebase.md`.
- If `scripts/gui-goldens.py publish` cannot reach the private goldens storage repository, block with the exact error
  rather than committing a broken manifest.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-codex-recipe-credentials-at-launch-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-codex-recipe-credentials-at-launch-watchdog.json`
beside it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the
agent scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted tests in `farhelm-proto`, `farhelm-supervisor` (`recipe_credentials`, `agent_kind`),
`farhelm-helm` (`agent_endpoints`, `credential_delivery_tests`), relevant e2e tests (`host_creates`,
`podman_environments` if Podman is available), `farhelm-gui` recipe editor tests, `check-gui-goldens.py` +
`gui-goldens.py publish`, website checks for edited docs, `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `dprint check` on changed Markdown, and
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to
every test you add or change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
- The last PR of the stack removes both TODO.md entries this plan covers ("Revisit deferring the Codex credential
  migration" and "Move Codex recipe endpoint credentials out of its settings"), as root `AGENTS.md`, "TODO.md and
  FLAKES.md", requires.
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
last removing both TODO.md entries this plan covers. Open, not merged. If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
