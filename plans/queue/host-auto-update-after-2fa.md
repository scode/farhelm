# Allow host auto-updates to prompt for SSH sign-in (2FA) and resume

Written against main at bd95a669 on 2026-11-22. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and where this plan changes what they specify, it says so and you update them in the same stack.

Depends on `remove-host-signin-lock.md` being on `main` first (both plans refactor `SetupSignIn`'s active-attempt map in
`crates/farhelm-helm/src/provisioning/setup_sign_in.rs`, and `remove-host-signin-lock.md` ensures removing a host
cancels active setup sign-in attempts without blocking).

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Trivial"). Verbatim:

> **Host auto-update may stall on 2FA prompts.** i THINK auto-updating of hosts may stop working when 2fa prompts are
> involved. at least i had to click in an environment where i also have a 2fa prompt. tbd during planning.

Maintainer note during selection:

> my suspicion is that there is a missing trigger of auto-upgrade after successful auth or something like that.

In product terms: make automatic remote-host updates work on SSH hosts that require 2FA or another interactive SSH
sign-in flow. When a host needs an automatic update and its separate setup/updates SSH connection needs interactive
authentication, open a quiet Setup SSH sign-in prompt in the attention row without blocking other hosts' automatic
updates and without stealing focus, and automatically plan and run the update once the user finishes signing in (while
remembering a cancelled, expired, or failed sign-in for that connection incarnation under the existing no-retry rule).
Also fix the protocol-skew (`VersionSkew`) connection incarnation bug so signing in or retrying a host whose protocol is
too old advances its incarnation and makes automatic updates eligible again.

### Where it stands (from planning research at main bd95a669; line numbers approximate)

- **Why automatic updates currently stall on 2FA hosts (`crates/farhelm-helm/src/ssh.rs:39-63, 147-188`,
  `provisioning/automatic.rs:147`, `provisioning/service.rs:1591-1697`):**
  - Farhelm uses two separate OpenSSH `ControlMaster` connections per host: `SharedConnection::Supervisor` (`%C`) for
    sessions and `SharedConnection::Provisioning` (`p%C`) for setup and updates (`SPEC.md:5826-5835`). Answering 2FA to
    connect the host authenticates only `Supervisor` (`%C`).
  - `AutomaticUpdates` wakes up on the resulting `FleetEvents` bump and calls `service.plan_update(host.id)` ->
    `plan_update_guarded(host, /* interactive = */ false)`, which skips `setup.prepare(...)` and probes `p%C` with
    `BatchMode=yes`. On a 2FA host, that probe fails immediately (exit 255 -> `BackendFailure`), and
    `automatic.rs:168-185` silently suppresses retries for the rest of that connection incarnation.
- **Secondary `HostState::VersionSkew` incarnation bug (`crates/farhelm-helm/src/manager.rs:4342, 6115-6155`):**
  - `HostActor::publish_refresh()` only increments `status.incarnation` when
    `!same_connection(status.client.as_ref(), client.as_ref())`. Because `VersionSkew` has `client == None`,
    transitioning `Connecting` / `SignInNeeded` / `Unreachable` -> `VersionSkew` (or explicit user `Retry` while already
    `VersionSkew`) leaves `status.incarnation` unchanged, so any earlier `attempted` or `failed_incarnation` suppression
    sticks until helm restart.
  - **Critical invariant:** Periodic 45-second background re-probes (`cadence.reprobe`, `VersionSkew` -> `VersionSkew`)
    must **never** bump `status.incarnation` (otherwise failed attempts would retry every 45 s in violation of
    `SPEC.md:188-190`, and a 45 s re-probe would invalidate a 120 s Setup 2FA prompt mid-flight).
  - Bumping `incarnation` when entering `HostState::VersionSkew` from any non-`VersionSkew` state covers initial
    connect, `SignInNeeded` -> **Sign in**, master-expiry re-sign-in, `Unreachable`, and `Connected` transitions. For
    explicit user **Retry** (`retry_now`) while already `VersionSkew` with a live `Supervisor` master, distinguish a
    non-provisioning user nudge (`wait_or_nudge` returning `Some(nudge)` with `!nudge.preserves_sign_in()`) from timer
    expiry (`None`), advance `status.incarnation`, and include `incarnation_changed` in `observable_change`
    (`manager.rs:6132`) so `events.bump()` fires and wakes `AutomaticUpdates`.
- **Reusing `plan_update_guarded` & `SetupSignIn` (`provisioning/service.rs:1607-1619`, `setup_sign_in.rs:123-204`,
  `http.rs:350-403`):**
  - Do **not** add a new `ProvisioningRequestError::PendingSshSignIn` variant: `automatic.rs` is a submodule of
    `service.rs` and can call `service.plan_update_guarded(host.id, interactive)` directly and downcast
    `super::setup_sign_in::Pending` (just as `http.rs:350` already does), which already releases `plan_slots` and
    `ssh_route_lock` before returning `Pending`.
  - Generalize `SetupSignIn::start` from `state: Arc<AppState>` to `service: Arc<ProvisioningService>` (all three fields
    it reads — `host_row`, `manager.ssh_route_lock`, `manager.events().bump()` — already live on `ProvisioningService`),
    updating its single HTTP caller at `http.rs:401` (`Arc::clone(&state.provisioning)`).
  - **Do not poll `SetupSignIn::completed` from `AutomaticUpdates`:** `completed` (`setup_sign_in.rs:28-29, 188-200`) is
    a bounded HTTP polling buffer (`RETAIN = 180s`, `MAX_COMPLETIONS = 128`) whose eviction would wedge a host. Instead,
    have each active `SetupSignIn` attempt hold a `tokio::sync::watch::channel<Option<SshSignInOutcome>>(None)` and
    return `watch::Receiver<Option<SshSignInOutcome>>` from `SetupSignIn::start` (while continuing to populate
    `completed` for HTTP and calling `service.manager.events().bump()` on settlement).
  - **Deduplication across `prepare()` and `start()` (required to avoid `SignIn::establish`'s 15 s `gate.lock()` timeout
    and missed GUI focus handoff):**
    - `askpass::Questions::open` bumps `FleetEvents`, waking `AutomaticUpdates` immediately when a manual Update or
      Uninstall opens a prompt (before `memory.busy` is claimed), and a user clicking manual Update while an automatic
      prompt is open expects the GUI to focus the existing prompt (`http.rs` serializes `Pending.attempt.id` before
      calling `start()`).
    - Extend `SetupSignIn`'s active tracking (building cleanly on top of `remove-host-signin-lock`'s unified `active`
      map, preserving `cancel_draft` and `cancel_host(HostId)`) so registered host entries also carry
      `(destination, access, outcome: watch::Receiver<Option<SshSignInOutcome>>, alias_attempts: Vec<String>, manual_attached: bool)`.
    - In `prepare()`: if an active attempt already exists for `row.id` with matching `(destination, access)`, return
      `Pending` carrying that existing `Attempt` and `existing_outcome: Some(rx)`.
    - In `start(pending, service, manual: bool) -> watch::Receiver<Option<SshSignInOutcome>>`: if `existing_outcome` is
      `Some(rx)`, mark `manual_attached |= manual` (if still active) and return `rx` without spawning a second `ssh`. If
      another caller won the `prepare()`-to-`start()` race for the same `HostId` and `(destination, access)`, push
      `pending.attempt.id` into `winner.alias_attempts`, set `winner.manual_attached |= manual`, and return
      `winner.outcome.clone()`. On settlement, record completions for `winner.attempt.id` and every alias ID in
      `completed`, send `Some(outcome)` on the watch channel, and bump `events()`.
    - Cancel auto-only attempts (`!manual_attached`) when `auto_update_remote_hosts` turns off (`SPEC.md:192`), when the
      host becomes ineligible or disappears, or when `AutomaticUpdates` stops/drops — without recording `Attempted` on
      opt-out/drop so turning auto-updates back on stays eligible.
- **Collapsed `AutomaticUpdates` state (`crates/farhelm-helm/src/provisioning/automatic.rs`):**
  - Replace the single `attempted` map with one per-host state map holding either
    `PendingSignIn { incarnation: u64, attempt_id: String, outcome: watch::Receiver<Option<SshSignInOutcome>> }` or
    `Attempted(u64)`. Do **not** add a separate `sign_in_tried` map (every exit from `PendingSignIn` either records
    `Attempted` on `Failed` or immediately runs `plan_update_guarded(host.id, /* interactive = */ false)` +
    `start_automatic_update` on `Ready`, whereas a separate `sign_in_tried` map would turn an opt-out skip into a
    consumed retry).
  - Run stale `PendingSignIn` cleanup (removed host, preference turned off, host no longer eligible, or
    `current.incarnation != pending.incarnation`) **before** the preference-off early return and `!eligible` skip,
    cancelling auto-only attempts on opt-out/ineligibility without recording `Attempted`.
  - When `plan_update_guarded(host.id, /* interactive = */ true)` returns `setup_sign_in::Pending`, capture
    `current_incarnation = service.manager.status(host.id).map_or(host.incarnation, |s| s.incarnation)`, start it via
    `setup_sign_in.start(pending, Arc::clone(service), /* manual = */ false)`, store `PendingSignIn`, and continue
    scanning the remaining hosts immediately without blocking.
  - On a subsequent scan when `outcome.borrow().clone()` is `Some(SshSignInOutcome::Ready)`: remove `PendingSignIn` and
    immediately run `plan_update_guarded(host.id, /* interactive = */ false)` + `start_automatic_update` (recording
    `Attempted(current_incarnation)` on refusal/start, or clearing if `Busy` / opt-out `Ok(None)`). On `Some(Failed)`
    (or closed watch channel): remove `PendingSignIn` and record `Attempted(pending.incarnation)`.
- **Desktop GUI (`crates/gui/farhelm-gui/src/sign_in.rs`, `session_list/attention.rs`, `sounds/owner.rs`):**
  - Already keeps unsolicited registered-host `Setup` questions (`startup: false`, no local `setup_mark`/`run_mark`)
    quiet in the attention row (with badge, attention cue, and SSH sign-in chime) without auto-opening or stealing focus
    — even right after answering a `Sessions` question on the same host — while still chaining follow-up prompts once
    the user clicks **Answer**. No GUI production edits are needed; add a focused regression test in
    `crates/gui/farhelm-gui/src/sign_in/tests.rs`.

### Acceptance criteria

1. Entering `HostState::VersionSkew` from any non-`VersionSkew` state (and explicit user `Retry` while already
   `VersionSkew`) advances `status.incarnation` and bumps `FleetEvents`, while periodic background `cadence.reprobe`
   checks (`VersionSkew` -> `VersionSkew`) never advance `status.incarnation`.
2. When an eligible remote SSH host needs an automatic update and its `SharedConnection::Provisioning` master is not
   live, `AutomaticUpdates` starts an async `SetupSignIn` without holding `plan_slots`/`ssh_route_lock` and without
   blocking other hosts in the fleet.
3. Completing the Setup SSH sign-in prompt (`Ready`) wakes `AutomaticUpdates` via `FleetEvents` and immediately re-plans
   (`interactive = false`) and starts the automatic update without prompting again; cancelling, failing, or letting the
   prompt expire (`Failed`) suppresses automatic retries for that connection incarnation and leaves the `↑ Update`
   badge.
4. Concurrent auto-update and manual Update/Uninstall sign-in checks for the same host deduplicate cleanly across
   `prepare()` and `start()` (no 15 s `gate.lock()` timeout; manual Update focuses the existing prompt; auto-update
   never cancels a `manual_attached` attempt), and turning off `auto_update_remote_hosts` cancels auto-only pending
   sign-in attempts without consuming the host's retry for that incarnation.
5. Focused GUI tests in `crates/gui/farhelm-gui/src/sign_in/tests.rs` confirm an unsolicited `Setup` question (in an
   untouched window and right after answering a `Sessions` question on the same host) stays in the attention row without
   stealing focus, and opens properly when the user clicks **Answer**.
6. `SPEC.md` ("Automatic remote-host updates" and "SSH sign-in") and `SPEC_impl.md` ("SSH sign-in") are updated, a
   changelog fragment is added, and the TODO.md entry is removed.

## Decisions already made

**The maintainer's request and decisions (2026-11-22):**

- When an automatic remote-host update needs SSH sign-in (2FA / password) on the setup/updates connection, open a Setup
  SSH sign-in prompt in the attention row and resume auto-update automatically once the user signs in (recording failure
  for that incarnation if cancelled/expired/failed).
- Desktop presentation: show in the attention row (with badge/sound as usual) without auto-opening or stealing focus.
- Fix the `HostState::VersionSkew` incarnation bug in this same plan.
- Review gate: single fresh-context agent on `gpt-6.2-sol` at `high` effort.
- Release note (verbatim):
  `Automatic updating of remote hosts now works even for remote hosts that require 2FA or other interactive sign-in flow.`

**Binding repository constraints:**

- `SPEC.md:181-194` (Automatic remote-host updates) and `SPEC.md:5826-5849` (Two shared SSH connections & SSH sign-in).
- `SPEC_impl.md:2739-2766` (SSH sign-in implementation).
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", "TODO.md and FLAKES.md", and
  "The live install is off-limits".

**Planner proposals (reviewed by fresh-context scope review):**

- Reuse `ProvisioningService::plan_update_guarded(host, interactive)` and `setup_sign_in::Pending` directly (no new
  `ProvisioningRequestError` variant).
- Generalize `SetupSignIn::start` to take `Arc<ProvisioningService>` and return a
  `watch::Receiver<Option<SshSignInOutcome>>` instead of polling the bounded HTTP `completed` buffer.
- Deduplicate registered-host `SetupSignIn` attempts across `prepare()` + `start()` (carrying existing `Attempt` +
  `watch::Receiver`, plus `alias_attempts`), and cancel auto-only (`!manual_attached`) attempts on opt-out,
  ineligibility, or listener drop without consuming the host's retry.
- Collapse `AutomaticUpdates` per-host tracking into one enum map (`PendingSignIn` | `Attempted`) with no separate
  `sign_in_tried` map.
- Also bump `status.incarnation` on explicit user `Retry` while already `VersionSkew` (with `incarnation_changed` in
  `observable_change`), while keeping periodic `cadence.reprobe` strictly non-bumping.
- Slice into 2 PRs: (1) `VersionSkew` incarnation fix + tests, (2) async `SetupSignIn` in `AutomaticUpdates` +
  deduplication + GUI quiet-attention-row test + specs + changelog fragment + TODO removal.

## Outline

1. `plan/host-auto-update-after-2fa/01-version-skew-incarnation`:
   - In `crates/farhelm-helm/src/manager.rs`, advance `status.incarnation` when entering `HostState::VersionSkew` from a
     non-`VersionSkew` state and on explicit non-provisioning user `Retry` while already `VersionSkew`, include
     `incarnation_changed` in `observable_change`, and keep periodic `cadence.reprobe` (`VersionSkew` -> `VersionSkew`)
     strictly non-bumping.
   - Add unit tests proving: entering `VersionSkew` from `Connecting`/`SignInNeeded` advances incarnation, explicit
     `Retry` while `VersionSkew` advances incarnation and wakes `FleetEvents`, and periodic `cadence.reprobe` does not
     advance incarnation.
   - Add a `kind: fixed` changelog fragment (or `kind: none` if folded into PR 2's fragment — prefer adding PR 2's
     combined user-facing `kind: fixed` fragment in PR 2 and a `kind: none` internal-support fragment in PR 1, or
     amending PR 1's fragment in PR 2 per `AGENTS.md`).
   - Title: `fix: advance host incarnation on reconnect or retry into version skew`.
2. `plan/host-auto-update-after-2fa/02-auto-update-setup-sign-in`:
   - Generalize `SetupSignIn::start` (`crates/farhelm-helm/src/provisioning/setup_sign_in.rs`, caller `http.rs:401`) to
     take `Arc<ProvisioningService>` and return `watch::Receiver<Option<SshSignInOutcome>>`, deduplicate registered-host
     attempts across `prepare()` + `start()` (with `alias_attempts` and `manual_attached`), and add cancellation for
     auto-only attempts while preserving `cancel_draft` and `cancel_host(HostId)`.
   - Wire `AutomaticUpdates` (`crates/farhelm-helm/src/provisioning/automatic.rs`, `service.rs`) with one per-host state
     enum (`PendingSignIn` | `Attempted`), running stale-pending cleanup before the opt-out early return / `!eligible`
     skip, starting async setup sign-in without blocking other hosts, resuming with `interactive = false` on `Ready`,
     and suppressing for the incarnation on `Failed`.
   - Add end-to-end helm provisioning tests (2FA setup sign-in -> `Ready` -> auto-update runs; `Failed`/cancel ->
     suppressed for incarnation; non-blocking scan across multiple hosts; opt-out cancels auto-only prompt without
     consuming retry; auto/manual deduplication; `VersionSkew` Sign in -> auto-update retries) and a focused GUI test in
     `crates/gui/farhelm-gui/src/sign_in/tests.rs` verifying quiet attention-row presentation without focus steal.
   - Update `SPEC.md:181-194, 5826-5849` and `SPEC_impl.md:2739-2766`, add/update the `kind: fixed` changelog fragment
     under `releasing/changelog.d/`, and remove the TODO.md entry ("Host auto-update may stall on 2FA prompts").
   - Title: `fix: prompt for SSH sign-in and resume automatic remote-host updates`.

## Fallbacks

- If `remove-host-signin-lock.md` landed with slightly different internal field names on `SetupSignIn::active`, build on
  the exact shape on `main@origin` while preserving `cancel_draft` and `cancel_host(HostId)`.
- Escalate via `queue block` only if a repository invariant conflicts with the settled behavior above.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-host-auto-update-after-2fa-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-host-auto-update-after-2fa-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run targeted `farhelm-helm` tests (`manager`, `setup_sign_in`, `automatic`, `provisioning` integration
tests in `crates/farhelm-helm/tests/provisioning.rs`), targeted `farhelm-gui` tests (`sign_in`),
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, `dprint check` on changed Markdown,
`python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py` as
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
- The last PR of the stack removes the TODO.md entry this plan covers ("Host auto-update may stall on 2FA prompts"), as
  root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
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
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

Automatic updating of remote hosts now works even for remote hosts that require 2FA or other interactive sign-in flow.
