# Stop Remove host from blocking on an open SSH sign-in prompt

Written against main at bd95a669 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Stop Remove host from blocking on an open SSH sign-in prompt.** PR #4638 (`tailscale-ssh-hosts`) added a per-host
> read-write lock (`ssh_route_lock`) so toggling Tailscale mode or removing a host does not race with an active SSH
> probe or sign-in check writing private `known_hosts` pins or starting a shared SSH master. Because a Set up, Update,
> or Uninstall sign-in prompt holds a read guard on `ssh_route_lock` while waiting for the user (up to about 2 minutes
> per prompt, reset on each new prompt) and `remove_host` awaits a write guard on the same lock for every SSH host (not
> just Tailscale ones), clicking Remove while a sign-in prompt is open silently blocks until the prompt times out or is
> answered, making Remove look stuck. Either make Remove fail fast as busy, cancel the open sign-in check, or check
> whether `remove_host` needs `ssh_route_lock` across the sign-in wait at all (editing a host's SSH destination already
> changes the route without taking it).

This plan also closes the matching queued review feedback item:
`review_feedback_queue/trailing-remove-ssh-host-blocks-on-ssh-route-lock.md` (and its entry in
`review_feedback_queue/INDEX.md`).

In product terms: make removing an SSH host (and Uninstall's final forget-host step) respond promptly without waiting on
an open setup, update, or uninstall SSH sign-in prompt, a host probe, or a background Podman check; immediately cancel
any open setup/update/uninstall SSH sign-in prompt for that host when it is removed (closing the prompt and ending its
SSH process, matching how Remove already handles session sign-in prompts); and ensure an in-flight discovery probe or
stale setup confirmation for a removed host can never re-insert a deleted host row.

### Where it stands (from planning research at main bd95a669; line numbers approximate)

- **Why `delete_registered_host` currently blocks (`crates/farhelm-helm/src/provisioning/service.rs:2573-2602`):**
  - `ProvisioningService::delete_registered_host` is shared by `DELETE /api/hosts/{id}` (`hosts::remove_host_for`),
    agent/sandbox removal (`hosts::remove_host_checked`, which has no 20 s reply bound), and Uninstall's final
    `ProvisioningAction::ForgetHost` (`service.rs:3469-3481`).
  - It awaits `self.manager.ssh_route_lock(host).write_owned().await` for every `HostKind::Ssh` row while holding
    `host_provision_lock(host)`. Three readers hold `ssh_route_lock(host).read_owned()` without holding
    `host_provision_lock`: `SetupSignIn::start` (`setup_sign_in.rs:147-164`, up to 120 s per interactive prompt),
    `ProvisioningService::lock_ssh_plan` (`service.rs:3718-3734`), and `PodmanPlaces::refresh`
    (`environment/podman_probe.rs:796-868`, triggered automatically when the launcher opens).
  - Unlike `hosts::set_tailscale` (`hosts.rs:993-1037`), `delete_registered_host` does **not** retire shared SSH
    masters, and `forget_tailscale_keys` (`service.rs:2616-2634`) is already best-effort per `SPEC_impl.md:6172-6173`
    (matching `hosts::retarget`). `hosts::set_tailscale` is already mutually excluded from `delete_registered_host` by
    `host_provision_lock(host)`, `PodmanPlaces::refresh` already fences `claim_podman_place` on `(id, entry_uuid)`, and
    `plan_update_owned`/`plan_uninstall_owned` never write to the store.
- **Unified cancellation in `SetupSignIn`
  (`crates/farhelm-helm/src/provisioning/setup_sign_in.rs:53, 123-204, 300-308`):**
  - `SetupSignIn` already cancels draft sign-in attempts via `drafts: Mutex<HashMap<String, oneshot::Sender<()>>>` and
    `tokio::select!` around `setup.wait(...)` (dropping `wait` SIGKILLs the SSH process group via
    `InteractiveChild::drop`, closes the `SshQuestion` via `QuestionLease::drop`, releases `ssh_route_lock`, records
    `SshSignInOutcome::Failed`, and bumps events), while `pending.entry: Option<(HostId, String)>` already distinguishes
    drafts (`None`) from registered hosts (`Some((host_id, _))`).
  - Generalize `drafts` into one map `active: Mutex<HashMap<String, ActiveAttempt>>` where `ActiveAttempt` holds
    `{ host: Option<HostId>, cancel: oneshot::Sender<()> }` (avoiding a second map and removing the
    `if let Some(cancel) ... else ...` branch in `start()`). Keep `cancel_draft(attempt)` restricted to entries with
    `host.is_none()`, and add `cancel_host(host: HostId)` that removes and fires every entry with `host == Some(host)`.
  - Do **not** extend cancellation to `SetupSignIn::establish_for_run`: it runs only inside accepted provisioning runs
    holding `host_provision_lock(host)`, so Remove already refuses immediately as busy and Uninstall's `ForgetHost` runs
    only after `establish_for_run` finishes.
- **Linearization order in `delete_registered_host` (`service.rs:2573-2602`):**
  - Remove the `ssh_route_lock(host).write_owned().await` wait from `delete_registered_host`.
  - Call `self.setup_sign_in.cancel_host(host)` **after** `self.store.remove_host_row(host, deleted).await?` commits
    (alongside `forget_host_memory` and `stop_actor`, before `forget_tailscale_keys`). Because `SetupSignIn::start`
    inserts into `active` synchronously before spawning the task that runs `lookup().await?`, calling `cancel_host`
    after `remove_host_row` guarantees either `cancel_host` finds and cancels the attempt, or the late-starting task's
    `lookup().await?` sees `HostNotFound` before `setup.wait(...)`, and failed deletions never cancel an unrelated
    sign-in.
- **Fence existing-host discovery & setup confirmations against row resurrection
  (`service.rs:980-1165, 1518-1550, 2645-2658`, `store.rs:7661-7802`):**
  - The only hard data race `ssh_route_lock` guarded for `delete_registered_host` was a concurrent probe against an
    already-registered SSH host (`stored.is_some()`) calling `register_probed_ssh_host_with_mode` and executing
    `INSERT INTO hosts` after the row was deleted. Note: never use `new_entry.is_none()` as the guard condition, because
    raw/legacy probes for unregistered destinations (`stored.is_none()`, `new_entry: None`) legitimately insert new rows
    (`provisioning/http.rs:48-50`).
  - Carry `existing_host: Option<(HostId, String)>` on `ProbeRegistration::Ssh` (populated from `stored` in
    `probe_owned` and from `row` in `registration_for_row` at `service.rs:3830`, keeping `PartialEq, Eq` working for
    `require_registration_unchanged` including `HostKind::Tensorlake`).
  - Thread `expected_host: Option<(HostId, &str)>` into `HelmStore::register_probed_ssh_host_with_mode` (reusing it
    instead of adding a duplicate store method; `register_probed_ssh_host` keeps passing `None`). When
    `Some((id, uuid))`, verify inside the transaction that the row `(id, entry_uuid)` still exists (else return
    `HostStoreError::HostNotFound(id)`, never falling through to `INSERT`) and still matches the probed destination
    (else return `HostStoreError::StaleAttempt { host: id }`), then run the existing update branch. This covers
    `probe_owned`'s `Supervisor` and `SkewedSupervisor` branches as well as `Absent` -> `PendingConfirmation::Add` ->
    `claim_register_and_start`.
  - In `forget_host_memory` (`service.rs:2645-2658`), also drop `PendingConfirmation::Add` entries whose
    `ProbeRegistration::Ssh { existing_host: Some((planned, _)), .. }` matches `host`.
  - In `claim_register_and_start` (`service.rs:1518-1531`), when `ProbeRegistration::Ssh` has
    `existing_host: Some((id, _))`, use `Some(*id)` directly as `existing` rather than scanning by destination so a
    stale confirmation cannot claim the `busy` slot of a newly re-added host at the same destination.
  - Update stale comments citing the old `ssh_route_lock` wait or non-cancellable registered preflights
    (`service.rs:2580-2581`, `setup_sign_in.rs:52, 135, 206`).

### Acceptance criteria

1. `ProvisioningService::delete_registered_host` no longer awaits `ssh_route_lock`; removing an SSH host (via
   `DELETE /api/hosts/{id}` or `remove_host_checked`) and Uninstall's `ForgetHost` complete promptly even while
   `SetupSignIn::start`, `lock_ssh_plan`, or `PodmanPlaces::refresh` holds `ssh_route_lock`.
2. `SetupSignIn` tracks both draft and registered-host attempts in one `active` map, keeps `cancel_draft` restricted to
   drafts, and exposes `cancel_host(HostId)`. Removing a host calls `cancel_host(host)` after `remove_host_row` commits,
   immediately closing any open setup/update/uninstall SSH sign-in prompt and ending its SSH process.
3. `hosts::set_tailscale` continues to take `ssh_route_lock.try_write_owned()` and all three readers continue to take
   `ssh_route_lock.read_owned()`, preserving Tailscale mode-toggle mutual exclusion unchanged.
4. A concurrent `probe_owned` (across `Supervisor`, `SkewedSupervisor`, and `Absent` -> `PendingConfirmation::Add` ->
   `claim_register_and_start`) started for an already-registered SSH host can never re-insert a deleted host row
   (returns `HostNotFound` if deleted, `StaleAttempt` if retargeted), while probing an unregistered SSH destination with
   `new_entry: None` still inserts as before.
5. Regression tests cover Remove during an open registered setup sign-in prompt (confirming prompt closes promptly),
   Remove while `ssh_route_lock` is held (as by `PodmanPlaces::refresh` / planning probe), Uninstall `ForgetHost` while
   `ssh_route_lock` is held, and concurrent `probe_owned` / stale `PendingConfirmation::Add` not resurrecting a deleted
   row.
6. The TODO.md entry ("Stop Remove host from blocking on an open SSH sign-in prompt") and the review feedback queue item
   `review_feedback_queue/trailing-remove-ssh-host-blocks-on-ssh-route-lock.md` (with its bullet in
   `review_feedback_queue/INDEX.md`) are removed in the same PR.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- Remove immediately and cancel any open setup/update/uninstall SSH sign-in prompt for that host (closing its prompt and
  killing its SSH process, matching how Remove already handles session sign-in prompts), rather than failing fast as
  busy or waiting on short probes.
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note: `No release note needed.` (no highlight; folded full list only).

**Binding repository constraints:**

- `SPEC.md:5166-5198` ("Waiting between operations on one host"): removing a host must respond promptly whatever that
  host is doing.
- `SPEC_impl.md:6171-6173` (Tailscale mode edits vs best-effort pin cleanup on forget) and `SPEC_impl.md:8177-8181`
  (shared `delete_registered_host` for Remove and Uninstall's `ForgetHost`).
- `review_feedback_queue/AGENTS.md`: remove the addressed feedback file and its `review_feedback_queue/INDEX.md` bullet
  in the same PR as the fix.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md", and
  "The live install is off-limits".

**Planner proposals (reviewed by fresh-context scope review):**

- Generalize `SetupSignIn::drafts` into one `active` map rather than adding a second map.
- Reuse `HelmStore::register_probed_ssh_host_with_mode` with `expected_host: Option<(HostId, &str)>` rather than adding
  a duplicate update-only store method.
- Keep the fix in **1 cohesive PR** (~100–150 lines of production code across `setup_sign_in.rs`, `service.rs`,
  `store.rs`), avoiding dead-code churn from introducing `cancel_host` before its only caller.

## Outline

1. `plan/remove-host-signin-lock/01-nonblocking-remove-and-signin-cancel`:
   - Generalize `SetupSignIn::drafts` (`crates/farhelm-helm/src/provisioning/setup_sign_in.rs`) into one
     `active: Mutex<HashMap<String, ActiveAttempt>>` (`{ host: Option<HostId>, cancel: oneshot::Sender<()> }`), keep
     `cancel_draft` limited to `host.is_none()`, add `cancel_host(HostId)`, and update stale comments.
   - Remove `ssh_route_lock(host).write_owned().await` from `ProvisioningService::delete_registered_host`
     (`crates/farhelm-helm/src/provisioning/service.rs`), call `self.setup_sign_in.cancel_host(host)` after
     `self.store.remove_host_row(host, deleted).await?` commits, and prune host-bound `PendingConfirmation::Add` entries
     in `forget_host_memory`.
   - Add `existing_host: Option<(HostId, String)>` to `ProbeRegistration::Ssh`, thread
     `expected_host: Option<(HostId, &str)>` into `HelmStore::register_probed_ssh_host_with_mode` (`store.rs`) to fence
     existing-host probes against re-inserting deleted or retargeted rows, and match `existing_host` directly in
     `claim_register_and_start`.
   - Add targeted unit/integration tests covering Remove during open setup sign-in, Remove during held `ssh_route_lock`,
     Uninstall `ForgetHost` during held `ssh_route_lock`, and deleted-host resurrection prevention across `Supervisor`,
     `SkewedSupervisor`, and `PendingConfirmation::Add`.
   - Add a `kind: fixed` changelog fragment under `releasing/changelog.d/`, remove the TODO.md entry ("Stop Remove host
     from blocking on an open SSH sign-in prompt"), and delete
     `review_feedback_queue/trailing-remove-ssh-host-blocks-on-ssh-route-lock.md` along with its bullet in
     `review_feedback_queue/INDEX.md`.
   - Title: `fix: stop removing an SSH host from blocking on open sign-in prompts`.

## Fallbacks

- None expected; escalate via `queue block` only if a repository invariant conflicts with the settled behavior above.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-remove-host-signin-lock-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-remove-host-signin-lock-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted `farhelm-helm` tests covering `provisioning`, `setup_sign_in`, `hosts`, `store`, and Tailscale
route-lock behavior
(`cargo nextest run -p farhelm-helm --lib -E 'test(setup_sign_in) | test(remove_host) | test(tailscale) | test(register_probed_ssh_host) | test(delete_registered_host)'`,
plus the `provisioning` integration tests in `crates/farhelm-helm/tests/provisioning.rs`), `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, `dprint check` on changed
Markdown, `python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py` as
`docs/test-sleep-check.md` prepares it. Apply `.agents/test-authoring.md` to every test you add or change. Never take a
state-changing action against the live Farhelm install on this machine (root `AGENTS.md`, "The live install is
off-limits").

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
- The last PR of the stack removes the TODO.md entry this plan covers ("Stop Remove host from blocking on an open SSH
  sign-in prompt") and `review_feedback_queue/trailing-remove-ssh-host-blocks-on-ssh-route-lock.md` (with its
  `review_feedback_queue/INDEX.md` bullet), as root `AGENTS.md`, "TODO.md and FLAKES.md", and
  `review_feedback_queue/AGENTS.md` require.
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
last removing the TODO.md entry and review feedback queue item this plan covers. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
