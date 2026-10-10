# Execute the uninstall-reload triage outcome: hosts-panel uninstall ends the host's private tmux server

Written against main at bd8d5d76 on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this outcome's own PR amends them as described below.

This plan has no dependency on another plan.

## The goal

Carry out the triage outcome recorded in root `TRIAGE_OUTCOMES.md` under the heading `uninstall-reload.md` (outcome
`fix spec+code`), exactly as root `AGENTS.md` section "Execute triage outcomes" prescribes: one reviewable commit, one
stable bookmark and one draft PR carrying both the spec and the code change.

The feature is the uninstall item in a remote host's menu in the hosts panel. Today it removes the supervisor's user
service and Farhelm's lib directory but deliberately keeps the host's private tmux server running, which costs a fragile
ordering (the unit file is removed before the stop, and the stop is only safe while systemd still remembers the unit's
`KillMode=process`) and leaves a tmux server running from a deleted binary. The maintainer decided the opposite:
uninstall ends the private tmux server too.

The ledger entry (Assessment, Decision, Completion criteria) is the authoritative statement of what the PR must do. Read
it and the feedback file `review_feedback_queue/uninstall-reload.md` before starting. This file adds the plan-time
decisions and the mechanism chosen to satisfy the entry; where the two seem to disagree, the ledger entry wins and the
disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- One draft PR exists that makes hosts-panel uninstall end the host's private tmux server, with SPEC.md and SPEC_impl.md
  amended in the same PR as described below.
- Local `farhelm uninstall` is unchanged.
- The PR removes the feedback file and its index line, updates its own `TRIAGE_OUTCOMES.md` Execution field, and passed
  the review gate.
- The PR is not marked ready, and nothing is merged.

## Requirement sources

**The user's decisions (2026-10-09 triage), in their words:**

- "we SHOULD stop the tmux. especially since we're already saying uninstall is refused with active sessions. Why are we
  leaving the tmuxes? they should be SPECed to be internal detail of farhelm, not a user promised surface." SPEC.md
  already says the private tmux server is an implementation detail, not an interface ("Confirmed 2026-09-28").
- On a session started on the host while the uninstall runs, which would now be ended with the tmux server: accept it,
  "because its simpler and an edge case not sufficiently important. a session JUST created is unlikely to be important
  anyway. but SPEC should say this is fine and recommended but not required, if somehow being more strict is simpler
  that's fine. So it should SPEC as 'this is sufficient', not mandating it's not more 'correct'."
- On local `farhelm uninstall`: "leave alone for now".

**The user's plan-time decisions (2026-10-09):** review gate "gpt-6.1-sol high reviewer, no swarm"; one plan per
outcome; no-workhorse mode, which `plans/AGENTS.md` requires.

**Planner proposals** are the mechanism and file list below. A fresh-context planning review checked them against the
code and recommended prescribing the reorder rather than leaving two candidates open.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Sharing the machine with other agents; Agent scratch space; The live install is off-limits —
never run uninstall or touch units against this machine's own Farhelm), `plans/AGENTS.md` (Executing one plan),
`review_feedback_queue/AGENTS.md` (Lifecycle), and `.agents/test-authoring.md` for test changes.

## Outline

Line numbers drift; find the code by name. Verified on main at bd8d5d76.

**Mechanism: reload before stop.** In `crates/farhelm-helm/src/provisioning/plan.rs`, `plan_uninstall` pushes the
actions disable, remove unit file, stop, daemon-reload, remove lib directory, forget host. Reorder to disable, remove
unit file, daemon-reload, stop. Once the unit file is gone and the user manager reloaded, systemd forgets the unit's
`KillMode=process` and falls back to `control-group`, so the stop ends everything in the unit's control group, including
the private tmux server the supervisor started. The doc comment on `plan_uninstall` (around its action list) records
that this was verified against a real user manager when uninstall was built. Session agents and tabs already have to be
ended before uninstall proceeds (it refuses while any session has not ended or any tab is open), so what the stop now
ends is the tmux server and the panes of ended sessions.

What goes with it, all in the same PR:

- The planning-time `KillMode` refusal in `crates/farhelm-helm/src/provisioning/service.rs` (search for `KillMode` /
  `unit_kill_mode`) and the stop step's refusal in `crates/farhelm-helm/src/provisioning/backend.rs` (`stop`) existed
  only to keep tmux alive; remove both, and the `--no-reload` rationale in `disable` (keep the flag only if it still
  serves a purpose, and say which in the comment). Check whether `UninstallInspection::unit_kill_mode` still has a
  reader afterwards; drop it if not.
- Retry semantics: each uninstall host command treats work already done as a skip, and a failed run continues from where
  it stopped. Make sure the reordered steps keep that (a retry after a failed stop finds the file gone and the manager
  already reloaded; the stop must still work or skip cleanly).
- Tests in `crates/farhelm-helm/src/provisioning.rs` and `provisioning/e2e.rs` that assert the old order or the
  `KillMode` refusals change with it. The real-transport uninstall test (search `provisioning.rs` for the test that
  checks the tmux server is still alive after uninstall) currently asserts the server SURVIVES; invert it to assert the
  server is gone. That test is the only real proof the mechanism works, so run it. If it shows the server survives the
  stop (the planning review found no reason to expect that), fall back to killing the private tmux server through its
  socket under the host's state directory after the stop, and log the DECISION.
- SPEC_impl.md, the paragraph starting "UNINSTALL is the third operation": describe the new order and that the stop ends
  the private tmux server; drop "The tmux server is not stopped by uninstall at all ..." and the ordering and `KillMode`
  refusal reasoning that existed only to protect it. Keep the reason the unit file is removed before stopping (a retry
  may proceed without a connection only once nothing can start the supervisor again). State the limit honestly: the stop
  ends what is in the unit's control group, so a tmux server started outside the unit (for example by a supervisor
  started by hand) is not reached.
- SPEC.md, the paragraph starting "Removing Farhelm from a remote host": say the private tmux server is ended too, and
  replace "Uninstall never stops or kills a session or terminal tab" with wording that keeps the refusal (it refuses
  while sessions or tabs are live, checked at planning and again at confirmation) and says a session started on the host
  while the removal runs may be ended with it: the confirmation-time check is sufficient, and a stricter check is
  allowed but not required. Also adjust the plan text the user sees, if it describes what is kept or removed (`plan.rs`
  renders "remove ... with the Farhelm binary and any private tmux in it"; check the rendered plan says the tmux server
  is stopped, without inventing new UI).
- Changelog fragment: this changes what uninstall does for someone running Farhelm (`fix:` or `feat:` per the
  user-visible effect; `kind: changed` is the likely fit).

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, clippy on the touched crate, and focused nextest
selections for the provisioning modules through `scripts/record-test-run.py` (with the pinned nextest and tmux setup
from `docs/test-run-evidence.md`), including the real-transport uninstall test. Any PR that changes Rust tests: run
`python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.
`dprint check` on the changed Markdown.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-uninstall-ends-tmux-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and the `TRIAGE_OUTCOMES.md` Execution field on the PR) rather than starting over. If it does not exist, this is a fresh
start. A plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/AGENTS.md` (Executing one plan, step 7) before any work.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain. The sub-agents `plans/AGENTS.md`
requires of every plan (the resume check, and the cold reads of a blocked question and of the report) are exempt from
the no-delegation demand and run as that file says, not through galaxy-brain.

### Resource watchdog

Immediately after activating galaxy-brain, start a resource watchdog as a background process (not an agent) and keep it
running for the whole run. Every 60 seconds it samples free space on the filesystems holding the checkout, the agent
scratch directory and `/tmp`, plus available memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat`
and `sysctl` on macOS). It writes each sample to a private heartbeat/status file in the scratch directory, and it emits
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PR when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. The bookmark is
  `plan/uninstall-ends-tmux/01-uninstall-ends-tmux`.
- One outcome, one commit, one bookmark, one draft PR, per root `AGENTS.md` (Execute triage outcomes). Within this run,
  if the PR needs correcting, restructure it rather than stacking a correction on top; that applies to a PR an earlier
  run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog); `kind: none` with a one-line reason is right when nothing changes for
  someone running Farhelm. Validate with `python3 releasing/check-changelog.py format`.
- The PR removes the feedback file `review_feedback_queue/uninstall-reload.md` and its line in
  `review_feedback_queue/INDEX.md` (or narrows them, where this file says so), and updates its own `TRIAGE_OUTCOMES.md`
  Execution field to `complete` with the jj change ID, bookmark and PR URL. Record the change ID and bookmark before
  creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, if it changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
its changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria: the `TRIAGE_OUTCOMES.md` entry, the outline above, and the user's
decisions. For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as
root `AGENTS.md` requires. Also ask the reviewer to check specifically that a retry of a partly failed uninstall still
continues correctly from every step under the new order. Where you disagree with a finding, decide on the merits and log
the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new uninstall action, killing tmux by socket
instead of through the reordered stop, a change to local `farhelm uninstall`, a new check for sessions started during
the uninstall; these are examples, not a blacklist), and whenever the same component has needed repeated corrective
review rounds, run a fresh-context review through galaxy-brain with this charter, supplying the ledger entry, the user's
decisions above, this outline, the current diff and the proposed departure (what changed, why it is necessary, and which
simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the final step order and how retries behave under it, whether the `--no-reload`
flag and `unit_kill_mode` survive, and the exact spec wording for sessions started during an uninstall, the Conventional
Commit type and changelog kind, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. In particular, local `farhelm uninstall` stays out of scope, and no stricter session check is required.
If the work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan,
step 10). If current code or specs have moved so that the recorded decision no longer applies, that is a question for
the user too, per root `AGENTS.md` (Execute triage outcomes); never re-triage the item yourself.

## Done criterion

The plan is complete when its one draft PR exists, meets the acceptance criteria above, and (if it changes code, tests
or scripts) passed the review gate, with its `TRIAGE_OUTCOMES.md` Execution field updated and its queue item removed or
narrowed as this file says. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through
the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
