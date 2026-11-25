# Global helm repository history for Managed checkout suggestions

Written against main at c1d85d8ab on 2026-11-24. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. `SPEC.md` and `SPEC_impl.md` stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one `TODO.md` entry (bucket "Tensorlake", subsection "v1", "Wave 0 — No-UX-design fixes"). Verbatim:

> **Fix Managed checkout repository suggestions for new sandboxes.** When `New sandbox` + `Managed checkout` is
> selected, repository suggestions scan an existing connected host (such as Local) rather than using the recipe's
> configured GitHub token.

During planning, the maintainer decided to simplify Managed checkout repository suggestions across the entire product
rather than patching sandboxes beside per-host scanning:

> We should just have a history of repositories the user wanted to check out. So a global history for the Helm makes
> sense to me, rather than per host. ... if we just look at the Helm, all of this complexity goes away, and I would be
> perfectly happy to just completely remove the scanning of the files on a host to begin with.

And confirmed backfilling previously launched repositories from the helm's existing tables in the schema migration so no
previously launched repositories are lost on upgrade.

### Acceptance criteria

1. **Helm-global GitHub repository history (with migration backfill):**
   - Add a helm-global repository history table (or equivalent durable global store) in `crates/farhelm-helm/src/store`
     recording each GitHub `owner/repo` launched through Farhelm's Managed checkout (from both ordinary hosts and
     disposable sandboxes/containers, Agent and Command tabs alike), ordered by most recent use, with a bounded cap
     (e.g., matching `GITHUB_REPO_RESULTS_CAP` or `MAX_LAUNCH_HISTORY`).
   - In the schema migration that introduces the table, backfill existing launched repositories from both
     `create_history_sessions.github_repo` and `environment_launch_history.github_repo` (preserving canonical
     `owner/repo` validation and most-recent ordering) so previously launched repositories are retained on upgrade.
   - Deleting a host or a per-session sandbox/container never erases entries from the global repository history.
2. **Remove host-side filesystem repository scanning completely:**
   - Remove `ControlMsg::GithubRepoSearch` and `ControlMsg::GithubRepoSearchResult` from `crates/farhelm-proto` (bumping
     `PROTOCOL_VERSION` and updating the protocol-version pin test and any protocol-version references), remove
     `SupervisorClient::github_repo_search` in `crates/farhelm-helm/src/client.rs`, and remove the supervisor's
     `github_repo_search` handler and directory walker in `crates/farhelm-supervisor`.
   - Simplify the helm's repository suggestions route to a one-shot global `GET /api/github-repositories` (matching
     `GET /api/environment-launch-history`'s pattern: read once on launcher open, returning the bounded global list from
     `github_repository_history`, with client-side filtering via existing `repository_choices`), dropping server-side
     `query`, `GithubRepositoriesReq`, `GithubRepositoriesBody`'s host/incarnation/scan fields, the 150 ms debounce, and
     `Latest<SuggestKey, ...>`.
   - Preserve the inline "Set checkout folder" setup prompt in the launcher when launching a Managed checkout on an
     **ordinary host** (`Local` or `SSH`) that has no checkout folder configured: supply `needs_checkout_root: bool`
     directly from the helm's local checkout settings via the existing per-host `LaunchHistoryReply`
     (`GET /api/launch-history?host=<id>`, which the launcher already fetches per host and which already carries
     `checkout_config_revision`) — or reuse existing `GET /api/hosts/{id}/checkout-root` — with zero supervisor RPC, and
     never require a checkout folder for `New sandbox` or `New container`.
3. **Simplify launcher GUI & remove stale scan-error/incomplete states:**
   - In `crates/gui/farhelm-gui/src/launcher/view/checkout.rs` (and `choices.rs`, `history.rs`, `goldens/launcher.rs`,
     tests):
     - Load repository suggestions once globally per launcher open without tying them to `connected_host()` or any
       hidden existing host, so picking a repo before switching hosts, targeting an offline host, or targeting
       `New sandbox` / `New container` all get the exact same instant suggestions.
     - Remove the dead `truncated`, `scan_error`,
       `"Repository search is incomplete; a valid owner/repo can still be selected"`, `SuggestKey`, and `SUGGEST_AFTER`
       debounce plumbing. Note that `LauncherScene::RepositoryRefusal` currently tests HTTP 504 read failure (not
       `scan_error`): either drop it to match `launch-history` and `environment-launch-history`'s silent degradation on
       read error, or retain it for read failures cleanly and log the choice as a `DECISION`.
     - Keep `gh:` search in the launcher's top search bar working cleanly with the global history.
4. **Specs, docs, and tests:**
   - Update `SPEC.md`, `SPEC_impl.md`, and `website/src/content/docs/docs/using/start-a-session.mdx` (and any other docs
     describing host directory scanning for Managed checkout suggestions) to reflect helm-global repository history
     without host directory scanning.
   - Add unit/integration tests covering the migration backfill from both `create_history_sessions` and
     `environment_launch_history`, recency ordering, survival across host/sandbox deletion, global suggestions while
     targeting `New sandbox` or an offline host, and ordinary-host `needs_checkout_root` preservation.
   - Remove the covered `TODO.md` entry in the final PR of the stack.

## Decisions already made

### Maintainer decisions (binding)

- **Direction:** Replace per-host repository suggestions with one helm-global repository history and remove host
  directory scanning completely.
- **Migration backfill:** Backfill existing launched repositories from `create_history_sessions` and
  `environment_launch_history` inside the schema migration so previously launched repositories are not lost on upgrade.
- **Review gate:** Single fresh-context agent with the general charter on `gpt-6.2-sol` at `high` effort.
- **Release note:** Exact maintainer wording settled in `## Release note` below.

### Binding repository constraints

- `AGENTS.md`: Never mutate process environment variables in tests; never touch the live Farhelm install; annotate every
  test sleep with `// sleep-ok:` passing `scripts/check-test-sleeps.py`; keep schema migrations with fresh-schema
  statements, version stamp, and historical-schema migration fixtures; bump `PROTOCOL_VERSION` when removing a wire
  message variant.
- `SPEC.md`: Never contact GitHub during repository suggestion lookup; keep ordinary-host managed-checkout inline
  checkout-folder setup working when no checkout root is configured.
- `website/AGENTS.md`: When touching `website/` docs as an incidental update alongside code, name the changed page in
  the report without starting a preview server.

### Planner-chosen implementation shape

- **Dedicated `github_repository_history` table in helm SQLite** (keyed by canonical `owner/repo` with `last_used_at` /
  monotonic sequence, backfilled once in the migration from `create_history_sessions` and `environment_launch_history`
  and updated on every managed-checkout launch admission, while keeping `create_history_sessions.github_repo` intact for
  `launch_history`) rather than joining `create_history_sessions` and `environment_launch_history` on every read. A
  dedicated table is tiny, guarantees suggestions survive host or sandbox deletion and ordinary-session aging, and
  decouples repository history cleanly from per-host folder history.
- **One-shot `GET /api/github-repositories` on launcher open + client-side filtering (`repository_choices`)**, matching
  `GET /api/environment-launch-history` in `crates/gui/farhelm-gui/src/launcher/view/history.rs`, rather than keeping
  debounced per-keystroke `POST /api/github-repositories` requests.
- **Carry ordinary-host `needs_checkout_root: bool` on `LaunchHistoryReply` (`GET /api/launch-history?host=<id>`)**
  (which the launcher already calls per host and which already carries `checkout_config_revision`) — or reuse existing
  `GET /api/hosts/{id}/checkout-root` — so `GET /api/github-repositories` stays completely host-free with zero extra
  round-trips.

## Outline

### Existing mechanisms reused

- `crates/farhelm-helm/src/store.rs` and `crates/farhelm-helm/src/store/environment_history.rs` (schema migration
  ladder, historical-schema fixture tests, `parse_github_repo` validation, launch admission recording).
- `crates/farhelm-helm/src/checkout_config.rs` (`resolve_checkout_config` / `checkout_config_for` for ordinary-host
  `needs_checkout_root`).
- `crates/gui/farhelm-gui/src/launcher/view/history.rs` (one-shot helm-global fetch on launcher open +
  `LaunchHistoryReply`).
- `crates/gui/farhelm-gui/src/launcher/view/checkout.rs` (`repository_choices` client-side filter, `gh:` search, inline
  checkout-folder setup field, checkout path preview for ordinary hosts).

### PR stack

1. **`plan/global-checkout-repo-history/01-global-repo-history-store`** — Add the helm-global
   `github_repository_history` table, schema migration backfill from `create_history_sessions` and
   `environment_launch_history`, recording on every managed-checkout launch admission (ordinary hosts and disposable
   environments, keeping `create_history_sessions.github_repo` intact for `launch_history`), and store + historical
   migration unit tests. (Avoids breaking `farhelm-gui` or adding throwaway shim fields between PRs.)
2. **`plan/global-checkout-repo-history/02-global-route-gui-and-remove-supervisor-scan`** — Switch
   `/api/github-repositories` to one-shot global `GET`, carry ordinary-host `needs_checkout_root` via
   `LaunchHistoryReply` (or existing `GET /api/hosts/{id}/checkout-root`), simplify the launcher GUI (dropping
   `SuggestKey`, `SUGGEST_AFTER`, `Latest`, `truncated`/`scan_error`), remove `ControlMsg::GithubRepoSearch` /
   `GithubRepoSearchResult` (`PROTOCOL_VERSION` bump) and supervisor `github_repo_search`, update `SPEC.md`,
   `SPEC_impl.md`, docs under `website/`, add changelog fragment(s), and remove the covered `TODO.md` entry.

## Fallbacks

- If an existing GUI golden test or screenshot scene (`docs/gpui/screenshot-scenes.json`,
  `crates/gui/farhelm-gui/tests/goldens.txt`) only existed to cover the removed `RepositoryRefusal` / incomplete-scan
  state, remove that scene cleanly and verify `farhelm-gui` tests pass. If a golden fixture needs regeneration beyond
  simple removal, follow the existing golden rules in `.agents/validation.md`.
- Scope-reassessment triggers (examples, not a blacklist): adding any network call to GitHub, changing per-host _folder_
  history (`folder_history`), or changing how checkout path preview (`POST /api/github-checkout-preview`) works on
  ordinary hosts.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-global-checkout-repo-history-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-global-checkout-repo-history-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation":

- Targeted Rust tests per PR (through `record-test-run.py` with pinned nextest on `PATH`):
  - `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib -E 'test(github_repo) | test(checkout) | test(protocol_version) | test(migration)'`
  - `cargo nextest run -p farhelm --test e2e -E 'test(github_checkouts)'`
  - `cargo nextest run -p farhelm-gui --lib -E 'test(launcher) | test(checkout) | test(goldens)'`
- Formatting, lints, and checks:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`
  - `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`
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
- The last PR of the stack removes the `TODO.md` entry this plan covers ("Fix Managed checkout repository suggestions
  for new sandboxes."), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires. Any deferred items added to `TODO.md` go
  under `## Plan follow-ups`.
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
alternatives considered: in particular the API route shape chosen for global suggestions and ordinary-host
`needs_checkout_root`, the history cap, and every review finding you decided not to follow. The maintainer will ask for
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

GitHub repository suggestions have been simplified. There is now a simple history kept globally for your entire Farhelm
based on past session launches. The previous system was more complicated and probably not worth describing.
