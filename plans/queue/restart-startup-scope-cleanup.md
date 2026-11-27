# Plan: Unblock supervisor startup Restart scope cleanup when replacement host answers

## Goal

Fix Linux supervisor startup pending-scope cleanup (`retry_pending_scope_cleanups` in
`crates/farhelm-supervisor/src/service/core/host_restart.rs`) so that when a failed Restart's unheld replacement
terminal host is still answering on its socket, startup sweeps the replacement agent's scope and attempts to shut down
that answering replacement host — retiring the `.host` launch record and `pending_scope_cleanups` database obligation
once both succeed — instead of skipping the obligation on every startup forever.

Acceptance criteria:

- When `retry_pending_scope_cleanups` encounters an unheld failed-Restart launch record whose recorded host answers
  (`AtRecordedSocket::Answered`), it verifies the answering host's `HostTag` matches
  `(session_id == id, role == Agent { generation })` (refusing an unreadable or mismatched tag), sweeps that
  generation's scope/tree, and shuts down the answering host over the already-settled connection.
- Once both scope cleanup and host shutdown succeed (and the ended socket is removed), startup removes the `.host`
  launch record and retires the `pending_scope_cleanups` row in `supervisor.db`. If either fails, the record and DB row
  remain so the next startup retries.
- `unwind_failed_host_relaunch` continues to treat `AtRecordedSocket::Answered` as an error so a live replacement host
  keeps a failed Restart ambiguous rather than restoring the previous terminal.
- `SPEC_impl.md` is updated to match the answering-host cleanup behavior.
- A regression test covers a failed Restart (deleted mid-relaunch with scope cleanup pending) whose replacement terminal
  host still answers across supervisor restart, proving the scope is cleaned, the host is shut down, and the record and
  DB obligation retire.
- The last code PR removes the
  `Unblock supervisor startup Restart scope cleanup when the replacement's terminal host still answers.` entry from
  `TODO.md`.

## Requirement sources and settled decisions

- **Original request (`TODO.md`):** Context from PR #5005: `confirm_failed_restart_host_gone` turns
  `AtRecordedSocket::Answered` into an error, causing `retry_pending_scope_cleanups` to skip cleaning the scope and
  retiring the DB row forever while `host_reclaim.rs` also skips the record. A host that answers has already finished
  starting its agent, so the late-start race no longer applies. Clean its scope when `settle_launch_record` returns
  `AtRecordedSocket::Answered` for an unheld failed-restart host, decide whether to also attempt to end the answering
  host so the record and obligation can retire, update `SPEC_impl.md` if needed, and add a regression test.
- **Settled maintainer decisions:**
  - Clean the scope AND attempt to shut down the answering replacement host (after verifying it is unheld and its
    `HostTag` matches this exact `(session_id, Agent { generation })`) so the `.host` record and
    `pending_scope_cleanups` DB obligation can retire once both succeed.
  - Review gate: fresh-context Opus 5.5 agent at high effort with the general review charter.
- **Binding repository constraints:** `SPEC_impl.md` (pending Restart scope cleanup), `AGENTS.md` ("Conventional
  Commits", "Changelog fragments", "Tests and validation"), and `plans/AGENTS.md`.
- **Planner-proposed choices (from fresh-context planning review):**
  - Factor one small private helper (`settle_failed_restart_host`) in `host_restart.rs` returning `Gone` vs
    `Answered { socket, connection, info }` so `unwind_failed_host_relaunch` and `retry_pending_scope_cleanups` do not
    duplicate ~30 lines of launch-record/socket settlement.
  - Shut down the answering host with `crate::hosts::stop::shut_down(*connection, &socket, StopLimits::default())`
    followed by `self.remove_ended_socket(&socket).await` on `HostOutcome::Stopped(_)` (reusing the exact pattern in
    `crates/farhelm-supervisor/src/service/session_hosts.rs`), rather than calling `end_launch_host(None, ...)` which
    reconnects, adds a TOCTOU window, and checks only `session_id` without `generation`.
  - Leave `crates/farhelm-supervisor/src/service/core/host_reclaim.rs` unchanged (`reclaimable_records` already cedes
    pending Restart launch records and sockets to `retry_pending_scope_cleanups`).
  - Single PR (`plan/restart-startup-scope-cleanup/01-unblock-answering-host-cleanup`).

## Implementation outline

Single focused PR extending `crates/farhelm-supervisor/src/service/core/host_restart.rs`:

1. Extract a small private helper (for example `settle_failed_restart_host`) from `confirm_failed_restart_host_gone`
   returning a two-variant local enum (`Gone` vs `Answered { socket, connection, info }`):
   - `Gone`: launch record absent, empty, or `AtRecordedSocket::Gone` (removing the stale socket via
     `self.remove_ended_socket(&socket).await`).
   - `Answered`: `AtRecordedSocket::Answered` for a socket confirmed not in `held_sockets`.
   - `Err`: non-regular file, unreadable record, held socket, starter fence failure, `NoAnswer`, or `NoInfo`.
2. Keep `unwind_failed_host_relaunch` rejecting `Answered` with the existing error (and drop its unused
   `&HashSet::new()` argument via a thin wrapper or direct call).
3. In `retry_pending_scope_cleanups`:
   - On `Gone`, keep the existing sweep -> remove `.host` record -> retire DB row flow.
   - On `Answered { socket, connection, info }`, decode and verify `info.tag` matches `session_id == id` and
     `role == HostRole::Agent { generation }` (refuse unreadable or mismatched tags without touching the host or
     retiring the obligation). Sweep the scope/tree (`reap_recorded_scope` / `reap_process_tree_with_podman_deadline` as
     before, respecting `restored`), and shut down the answering host over `*connection` using
     `crate::hosts::stop::shut_down(*connection, &socket, StopLimits::default())` plus
     `self.remove_ended_socket(&socket).await` on `HostOutcome::Stopped(_)`. (If the scope sweep fails, deciding whether
     to skip host shutdown or still attempt it before keeping the record and DB row is a routine local fork; either way,
     only remove the `.host` record and retire the DB row when both scope cleanup and host shutdown succeed.)
4. Update `SPEC_impl.md` (the pending Restart scope cleanup paragraph describing startup's check on the recorded host).
5. Add a regression test exercising a failed Restart whose replacement terminal host still answers across supervisor
   restart (confirming scope cleanup, answering-host shutdown, launch-record removal, and DB retirement), plus a guard
   case if helpful (mismatched/unreadable tag left untouched).
6. Amend the unreleased PR #5005 fragment `releasing/changelog.d/retry-failed-restart-cleanup.md` per `AGENTS.md`,
   "Changelog fragments", and remove the covered entry from `TODO.md`.

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
   `cargo xtask records path plan/restart-startup-scope-cleanup` (`plans/.agents/outside-files.md`, "Files outside the
   repository"). Immediately invoke the `agent-resumeable` skill with that directory's `log.md`. If `log.md` already
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
  `farhelm-plan-restart-startup-scope-cleanup-watchdog.json` in the parent directory of the executing checkout (never
  inside the `plan/restart-startup-scope-cleanup` record directory). Watch the executing checkout root, `$HOME`, `/tmp`,
  the parent directory holding the status file, your scratch directory, and any worktree or build directory on another
  filesystem, at a 60-second interval with default alert levels. Follow `scode-build-goal`, "What the goal file must
  contain" (Resource watchdog) for verification, monitoring, `watchdog check` before launches and at boundaries, acting
  on alerts, and stopping monitors at close.

## PR discipline, validation, and review gate

- **Stack and bookmarks:** Build a linear stack of draft PRs with `jjstack`, based on `main@origin` (or this plan's own
  open `plan/restart-startup-scope-cleanup/*` PRs when resuming, never another plan's PRs), set up per
  `plans/.agents/executing.md`, "Executing one plan". Name bookmarks
  `plan/restart-startup-scope-cleanup/<nn>-<short-name>`. Keep PRs as drafts; never mark ready and never merge.
  Restructure a PR that needs correcting rather than stacking a fix on top, including across earlier runs' open PRs.
  Never edit `plans/`.
- **Conventions and validation:** Follow `AGENTS.md` ("Conventional Commits", "Changelog fragments", "Tests and
  validation"), `.agents/validation.md`, `.agents/test-evidence.md`, `.agents/test-authoring.md`, and the
  docstring/comment rules with `scode-voice`. Run targeted supervisor and restart/reclaim e2e selections from
  `.agents/validation.md` through `python3 scripts/record-test-run.py` (confirming no unintended `SKIPPED` for systemd
  scope coverage), plus `cargo fmt`, `cargo clippy`, `dprint check`, `python3 releasing/check-changelog.py format`, and
  `python -B scripts/check-test-sleeps.py`.
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
