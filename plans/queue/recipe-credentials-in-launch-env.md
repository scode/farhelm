# Recipe credentials through the launch environment

Written against main at 3617ee450 on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Tensorlake"). Verbatim:

> **Deliver recipe credentials through the agent's launch environment instead of its settings files.** Today Farhelm
> merges a recipe's credentials into Claude Code's `~/.claude/settings.json` and Codex's `~/.codex/config.toml` on the
> host, at setup, on each connection whose delivery is out of date, and when a named token is replaced, so it can race
> the agent or the user saving the same file (SPEC_impl.md, "Adding Tensorlake hosts", accepts the last few milliseconds
> of that race). Most of it could move into the environment Farhelm launches agents and tabs with instead: Claude's
> `ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN` and `CLAUDE_CODE_OAUTH_TOKEN` are plain environment variables, and
> Codex's provider can be selected with `-c` overrides at launch and read its token from an environment variable
> (`env_key`), never from the command line. Codex Subscription stays in `~/.codex/auth.json`, which Codex owns and
> refreshes. Costs to weigh: a `claude` or `codex` started in a shell Farhelm did not start no longer gets the
> credentials; a token rotation reaches agents only when they next launch; keys earlier releases wrote into the settings
> files need a one-time cleanup; and the recipe, delivery and Settings help text in the specs is rewritten. Container
> hosts share recipes once `plans/queue/podman-environments.md` lands, so the same design should cover them. From the
> 2026-11-10 triage of `settings-save-races-final-replacement.md`. Plan:
> `plans/queue/recipe-credentials-in-launch-env.md`.

`podman-environments` landed on 2026-11-11, so containers already go through the same delivery code: one design covers
sandboxes, sandbox sessions, persistent Tensorlake hosts and containers, with no per-provider code.

This plan runs after `agent-hook-setup.md`, which reshapes the same launch code (`prepare_launch`,
`AgentIntegration::farhelm_args`, the hook policy); that plan's work must be on main first.

### Where it stands (from planning research at main 3617ee450)

- **Recipe credential kinds** (`crates/farhelm-helm/src/provisioning/credential_delivery.rs`, `Bindings`, about lines
  43-49): Claude endpoint (base URL plus optional endpoint-token name), Claude login (setup-token), Codex endpoint,
  Codex login (`auth.json` contents), GitHub token. Claude endpoint and login are exclusive, as are Codex endpoint and
  login.
- **Helm delivery** is one shared `CredentialDelivery`: `setup` (about 358), `connect` (about 342, after `enter` takes a
  lease; called from `tensorlake/hosts.rs` and `environment/podman.rs`), `push` on replacement (about 403, from
  `credentials.rs`). `apply` (about 544) holds a per-host mutex and a nonsecret per-host `Record` (the
  `hosts.tensorlake_credential_delivery` JSON column, about 112-121) of stamps so an unchanged reconnect does no remote
  work. Slots, in order: Claude settings, Codex config (bounded read, merge on the helm, checked atomic install, one
  re-merge on a concurrent change), Codex auth (write-only), GitHub token (`<state_dir>/github-token`).
- **The merges** in `crates/farhelm-helm/src/provisioning/agent_endpoints.rs`: `claude` writes `ANTHROPIC_BASE_URL` and
  `ANTHROPIC_AUTH_TOKEN` into settings `env` and strips `CLAUDE_CODE_OAUTH_TOKEN`, `ANTHROPIC_CUSTOM_HEADERS`,
  `ANTHROPIC_API_KEY` and `apiKeyHelper`; `claude_subscription` writes `CLAUDE_CODE_OAUTH_TOKEN` and strips the endpoint
  keys; `clear_claude` removes the three keys; `codex` sets `model_provider = "farhelm"` and a
  `[model_providers.farhelm]` table with `experimental_bearer_token`; `clear_codex` removes them.
- **The precedent to follow with a sibling reader, on the supervisor:** `crates/farhelm-supervisor/src/github_token.rs`
  (`host_file`, `read_file` with fixed errors that never quote contents, `inject`). It is read afresh after
  `host_child_env` (`service/core/host_launch.rs`, about 141) at agent spawn (about 511, which Restart also uses) and
  tab spawn (`service/core/host_tabs.rs`, about 602), putting the value in `ChildSpec.env`, which reaches the terminal
  host only on its stdin (`crates/terminal-host/SPEC.md`: "the child's environment may hold credentials"). The
  spare-clone builder reads it too (`service/spare_builder.rs`). SPEC_impl.md (about 5935): "LaunchSpec, launch records,
  host argv/environment never carry this value."
- **What must not carry secrets:** `prepare_launch` (`service/core.rs`) writes the per-launch `LaunchSpec` to disk with
  argv and `FarhelmArgs.env` (`agent_kind/mod.rs`). Secrets go only through `ChildSpec.env`.
- **Spec text to rewrite:** SPEC.md about 416-420 ("Claude Code's authentication is merged into its settings'
  environment …"), 509-516 (Claude Subscription writes `CLAUDE_CODE_OAUTH_TOKEN` in settings.json; keep the sentence
  that Farhelm writes no first-run or onboarding metadata), 4872 (the 1 MiB settings-file bound becomes cleanup-only);
  SPEC_impl.md about 5915-5935 and 6425-6457 ("Adding Tensorlake hosts": the merge rules and the accepted race). SPEC.md
  "Security" (about 4966-4970) already allows "Farhelm's own state there".
- **User-facing text:** the recipe editor help (`crates/gui/farhelm-gui/src/settings/recipes/editor/credentials.rs`,
  about 109 and 125) and the docs page `website/src/content/docs/docs/using/tensorlake-sandboxes.md` (about 135-147).

### Acceptance criteria

1. Recipe Claude endpoint, Claude login and Codex endpoint credentials are no longer written into Claude's or Codex's
   settings files. The helm writes them into one private (0600) Farhelm-owned file in the supervisor's state directory,
   through the existing stdin uploader, under the existing delivery semantics (lease, mutex, stamps, warnings, push on
   replacement, "Saved; not delivered to ‹host›", no periodic retry).
2. The supervisor reads that file afresh, **once per spawn**, at every agent spawn, Restart and terminal-tab spawn, and
   puts exactly the variables it holds into `ChildSpec.env` of **every** session and terminal it starts on that host
   (Decisions); the same read supplies the Codex options, so argv and environment never come from two versions of the
   file. No stripping of other ambient credential variables: the helm rewrites the file whole, so the previous mode's
   variables are simply gone, and credentials already on the host stay "undefined results" (SPEC.md about 518-520).
   Secrets never appear in argv, the `LaunchSpec`, launch records, logs or `FarhelmArgs.env`; secret-bearing types get
   no `Debug`.
3. Codex endpoint mode reaches Codex sessions Farhelm launches through `-c` launch options that select a fresh provider
   id (not `farhelm`, so a leftover table cannot merge in) and name the token's environment variable (`env_key`); the
   options carry no secret. A `codex` typed by hand in a terminal uses its own configuration, and the help text says so.
   The per-kind options live where the `agent_kind` module map puts per-kind supervisor decisions, independent of
   whether hooks are on, spliced only on the `{farhelm_args}` path: legacy sessions (the `inject_hooks` path) predate
   Tensorlake and containers and can never run on a host that has the file.
4. Codex Subscription (`~/.codex/auth.json`) and the GitHub token (its own `github-token` file, which also drives gh's
   git helper set-up and the spare-clone builder) are unchanged.
5. A one-time cleanup removes the keys earlier releases wrote (reusing `clear_claude`/`clear_codex`), **only** on hosts
   whose stamp records a previous Farhelm write (Endpoint, Subscription, or Absent); for a slot whose stamp is unknown
   (every new host), record "cleanup done" without touching the files. A new stamp records completion so the settings
   files are never touched again. Recipe mode None therefore stops removing Claude keys that came with the image; the
   SPEC.md 509-516 rewrite says so.
6. The helm–supervisor protocol version is bumped (Decisions) in PR 2. The guarantee is: an older supervisor is refused
   at the hello, so it never starts a session, Restart or tab for this helm after the cleanup (setup's delivery runs
   before the hello and needs no extra gate). This is the first bump for a host-side contract rather than a wire change,
   so PR 2 also amends `PROTOCOL_VERSION`'s doc comment (`crates/farhelm-proto/src/lib.rs`, about 214-270: "Bumped only
   for incompatible frame or message changes") and SPEC_impl.md "Version and skew" (about 8315) to say a host-side
   contract the helm relies on also earns a bump, adds a "Protocol N …" paragraph there after the Protocol 79 one, and
   gives the reason in the commit message. Mechanically a bump touches the constant and the pinned-version test
   `protocol_version_is_pinned_at_N` (rename it, update its reference in the constant's doc, add a sentence to its doc
   list); the e2e skew tests use `PROTOCOL_VERSION - 1` and no fixture holds the number. Bump from whatever number main
   has when you get there (`agent-hook-setup` and `agent-trust-every-folder` bump too) and renumber on rebase per
   `.agents/careful-rebase.md`. The changelog fragment says nothing about updating remote hosts (from v0.40.0 that goes
   unsaid); the report's "things you should know" names the bump for release testing.
7. A token replacement reaches each session the next time it starts or restarts; running processes keep their
   environment, as for GitHub today. The recipe help text and docs say so.
8. Before relying on them, the agent behaviors under "Planner proposals" are checked on the installed agents with
   `scripts/agent-tui.py` (`.agents/real-agent-tui.md`), using an isolated `CLAUDE_CONFIG_DIR` / `CODEX_HOME` in scratch
   and a local stub endpoint (`scripts/agent-tui.py start -- env VAR=… claude` works, since the command takes the rest
   of the argument list); never edit the real `~/.claude/settings.json` or `~/.codex/config.toml`. The evidence goes in
   the report.
9. SPEC.md, SPEC_impl.md, the recipe help text and the docs page describe the new delivery. Tests: supervisor (fresh
   read per spawn, no content in errors, absence leaves the environment alone, a Codex launch's argv has the options but
   no token, the LaunchSpec on disk holds no secret) and helm delivery (the new slot, the cleanup rule including the
   unknown-stamp case, stamps, push), the tests that prove delivery happens before the supervisor is enabled
   (`crates/farhelm-helm/src/provisioning/tensorlake_add_tests.rs`, through the fake backend's `read-agent-settings` and
   `wrote-claude-settings` events in `provisioning/e2e.rs`; they need a marker for the new file), and the container e2e
   assertions that read settings.json today (`crates/farhelm/tests/e2e/podman_environments.rs`, about 570-575).
   (`host_creates.rs` checks no settings file.) The TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Every child gets the Claude variables**: every session and terminal Farhelm starts on that host, like the GitHub
  token. Other tools that read `ANTHROPIC_*` pick up the recipe's endpoint too; that is accepted.
- **Codex endpoint: launch options only.** A hand-typed `codex` uses its own configuration, and the help text says so.
  No settings-file writes remain.
- **Older supervisors: bump the protocol** so hosts with an older supervisor must update before connecting (not a
  build-version gate, and not a second delivery path).
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort. No visual review (text-only UI change).
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:** SPEC.md "Security" (never logged, never on a command line); SPEC_impl.md's GitHub
token paragraph as the precedent; root `AGENTS.md`, "Code rules" (per-harness behavior goes where
`crates/farhelm-supervisor/src/agent_kind/mod.rs`'s module docs say, never `kind == X` in shared code; no per-provider
code, per `crates/farhelm-helm/src/environment/mod.rs`); the protocol-version rules SPEC.md and SPEC_impl.md give for
helm–supervisor compatibility; `crates/terminal-host/SPEC.md`'s "Change policy" if you touch it (you should not need
to); `.agents/real-agent-tui.md`; `.agents/test-authoring.md`; root `AGENTS.md`, "Conventional Commits", "Changelog
fragments", "Tests and validation", and its "Instructions by path".

**Planner proposals:**

- Reuse `CredentialDelivery` as is; only the destination changes. Replace the Claude-settings and Codex-config slots
  with one stamped slot writing the new file (for example `<state_dir>/agent-credentials`, a small JSON map of
  environment variables plus the Codex provider facts). Keep `github-token` separate and unchanged. The per-agent
  variable names stay in the helm (`agent_endpoints.rs` owns them), so the supervisor's injection stays generic.
- A sibling reader next to `github_token.rs` (do not convert `github_token.rs` to the new format; sharing a tiny "read a
  private state file with fixed errors" helper is optional), read once per spawn in the spawn paths (`host_launch.rs`
  about 470-516, `host_tabs.rs` about 602); a malformed file refuses the spawn, as the GitHub precedent does.
- On the helm, the new slot reuses `install_bytes` as the GitHub slot does; for removal, generalize the backend's
  `remove_github_token(target, path)` (`provisioning/backend.rs`, about 339 and 2329; fake in `provisioning/e2e.rs`,
  about 250), which already takes a path, into a path-generic owned-file removal used by both slots, rather than adding
  a parallel one. If the variable name for the Codex token starts with `FARHELM_`, it joins the compatibility surface in
  SPEC_impl.md (about 8431); prefer a name that does not, or account for it.
- New `Record` fields are `Option`s inside the existing JSON column, so no schema migration.
- **Agent behaviors to check first** (inferences, not facts): (b) `-c model_provider=…` plus an inline
  `model_providers.<id>={…}` table override `config.toml`, profiles included; (c) a custom `wire_api = "responses"`
  provider sends `env_key`'s value as the bearer token; (d) whether a running Claude Code reloads its settings `env`
  when the cleanup removes keys (if it does, the upgrade's cleanup cuts off credentials for Claude sessions already
  running on environment hosts until they restart, a caveat for the fragment and the docs). (a) Whether Claude's
  settings `env` overrides the process environment is informational only: the cleanup runs regardless. If (b) or (c) is
  false, adapt within the decisions (log it); if the decisions cannot hold, block.

## Outline

1. `plan/recipe-credentials-in-launch-env/01-supervisor-credential-env`: `refactor:` (no helm writes the file yet) the
   supervisor reader and injection at agent spawn, Restart and tab spawn, the per-kind Codex launch options and their
   splice in `prepare_launch` on the `{farhelm_args}` path, and the supervisor tests.
2. `plan/recipe-credentials-in-launch-env/02-helm-delivers-launch-env`: `feat:` (or `fix:`; your call) the helm's new
   slot, the stamped one-time cleanup, removal of the endpoint and subscription merges, the protocol bump, helm and e2e
   tests, SPEC.md and SPEC_impl.md, the recipe help text and the docs page. Changelog fragment (`kind: changed`), whose
   context paragraphs mention the restart-to-rotate caveat and that hand-typed `codex` keeps its own configuration.
   Removes the TODO entry.

## Fallbacks

- PR 1 may be folded into PR 2 (log it).
- The TODO entry "Skip Claude Code's first-run setup on new Tensorlake hosts" stays in TODO.md and is not this plan's
  work; keep SPEC.md's "Farhelm writes no first-run or onboarding metadata" true.
- `triage-helm.md` (its credential push PR) edits nearby delivery code; if it has landed by the time you start, rebase
  carefully onto it.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-recipe-credentials-in-launch-env-log.md` in the parent directory of the checkout you run in, derived as
that section says, and the watchdog's status file is `farhelm-plan-recipe-credentials-in-launch-env-watchdog.json`
beside it. Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the
agent scratch directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/recipe-credentials-in-launch-env/01-supervisor-credential-env`,
  `plan/recipe-credentials-in-launch-env/02-helm-delivers-launch-env`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Deliver recipe credentials through the agent's
  launch environment instead of its settings files"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
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
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The maintainer will ask for these later.

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
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.

## Decisions

### 2026-11-22: answer to a blocked question

The question, as the executor put it:

> Blocked on 2026-11-22 (claim 442cdc).
>
> ### Keep existing Codex conversations resumable during credential migration
>
> Farhelm currently writes a recipe's endpoint token under the `farhelm` provider ID in Codex settings. The plan moves
> the token into Farhelm-launched processes and selects a fresh provider ID to keep stale settings from taking effect. A
> stale bearer can make the endpoint reject the user's next request even when Farhelm has delivered a replacement token.
>
> Codex 0.171.0 saves the provider ID with each conversation. I created a conversation with the existing ID, stopped
> Codex, and removed the old token and provider selection from the fixture's settings. In the isolated loopback test,
> `codex resume <thread-id> --no-daemon -c model_provider=farhelm_recipe` refused the saved conversation even with both
> provider definitions supplied. Providing both definitions without selecting one led to the sign-in screen. Selecting
> `farhelm` with the endpoint and token-variable name supplied through `-c` resumed the conversation and the new
> loopback endpoint received the new fake bearer. This is a direct TUI observation on one installed version. The
> successful case followed cleanup of the known old token and did not exercise other custom settings.
> [Codex 0.171.0 resume implementation](https://github.com/openai/codex/blob/rust-v0.171.0/codex-rs/app-server/src/config_manager.rs#L500-L565)
>
> The plan must preserve Restart for Codex sessions saved under the old provider. The same constraint applies when a
> user opens an existing conversation by ID from the launcher. Keeping the existing provider ID for new launches and
> resumes is the smallest option. Farhelm would still move token values into the child environment and clean the
> settings keys it previously wrote. Before delivery, verify that a stale setting cannot override the environment
> credential. Keeping the fresh ID for new launches would require a separate way to configure the saved provider for
> both forms of resume, including conversations whose ID was entered by hand.
>
> I recommend retaining the existing provider ID and verifying isolation from stale settings. The first draft prepares
> child environment injection. Which provider choice should the plan use for the delivery and cleanup change?
>
> [Draft #4558](https://github.com/scode/farhelm/pull/4558/changes) passed both focused spawn tests (recorder run
> `e903603d-eeab-42e5-a7f1-526ad9d66a2f`) and strict Clippy on the affected packages.

The maintainer's answer:

### Answer: keep the existing Codex provider ID

The landing monitor answered this block on the maintainer's behalf, under `plans/.agents/landing.md`, "Answering
blocks". A later change of mind is a follow-up, not a reason to stop the plan.

Keep the existing `farhelm` provider ID for new Codex launches and for resumes, as recommended. Restart and opening a
saved conversation by ID must keep working for Codex conversations saved under that ID, including ones whose ID the user
types into the launcher, and a fresh ID would break both unless the plan built a second way to configure the saved
provider.

The rest of the delivery change stays as planned: the recipe's endpoint token moves into the environment of the
processes Farhelm launches, and Farhelm removes the settings keys it wrote earlier. Before delivery, verify that a stale
setting cannot override the credential Farhelm delivers. Your experiment showed that supplying the provider's endpoint
and token-variable name with `-c` at resume used the new bearer; passing the same `-c` overrides on every Codex launch
and resume is an acceptable way to make the delivered values win over a leftover file entry. If verification shows a
stale setting can still win even then, block again with what you observed.
