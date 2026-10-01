# Execute the second 2026-10-01 triage batch: event feed, sink shutdown, duplicate hosts

Written against main at f82eaae on 2026-10-01. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Carry out the 6 triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/triage-feed-sink-identity.md` ``, exactly as root `AGENTS.md` section "Execute triage outcomes"
prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack. The stack
order, which is ledger order except that the Delete change moves last because it depends on a spec clarification from
another plan (see its item):

1. `plain-retry-erases-pending-fresh-window.md` (discard)
2. `event-feed-liveness-postponed-by-revisions.md` (fix code)
3. `input-client-notifications-pile-up.md` (other: `BUGS.md` entry)
4. `adopt-checks-current-row-not-dialed.md` (other: discard plus a `FILTER.md` filter)
5. `identity-mismatch-never-becomes-duplicate.md` (fix spec+code)
6. `sink-shutdown-retries-forever-after-delete.md` (fix code)

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms chosen to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- 6 draft PRs exist, one per outcome, stacked in the order above on top of the plan stack's tip (see PR discipline).
- Each PR does what its ledger entry's Completion criteria say, removes its feedback file and its line in
  `review_feedback_queue/INDEX.md`, and updates its own `TRIAGE_OUTCOMES.md` Execution field to `complete` with the jj
  change ID, bookmark and PR URL (record the change ID and bookmark before creating the PR, then add the URL to the same
  change and push again; no separate bookkeeping PR).
- Every PR except 1 has passed the review gate.
- PR 6 also changes this plan's line in `plans/INDEX.md` to `[executed]`. Do not delete this plan file.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "lets use the planning system to queue up execution of what we just triaged".

**The user's triage decisions (2026-10-01),** recorded in full in each ledger entry's Decision field. In short:

- Item 1: discard; a plain retry's downgrade of a pending fast reconnect only changes timing, never which machine is
  dialed.
- Item 2: fix, because the fix is trivial.
- Item 3: do not fix; document in `BUGS.md` as a known bug not planned to be fixed.
- Item 4: discard the finding and add a `FILTER.md` filter for races that need a person to act inside a self-closing
  sub-second window, whose whole consequence is recoverable through ordinary use, with the exclusions the ledger lists.
- Item 5: the user wants the simplest safe handling of a rare case, with less code than today: an entry that reaches a
  machine another entry already holds connects nothing, says which entry holds it by name, and tells the user to remove
  that entry or change this one's destination and then press Retry. Check "held by another entry" before comparing with
  the remembered identity. Drop the duplicate state's automatic 45-second re-check. Replace SPEC.md's "Two destinations
  reaching the same identity are the same host, shown once" with that rule.
- Item 6: Delete waits for the sink's orderly shutdown before killing the tmux session.

**The user's plan-time decisions (2026-10-01):**

- P1. Review gate: a fresh-context Opus 5.5 agent at high effort with the general review charter. No review swarm.
- P2. Only PR 1 (the discard) skips the review. PRs 3 and 4 (`BUGS.md`, `FILTER.md`) are reviewed.
- P3. Item 6's wait is bounded at about 5 seconds; if the sink's shutdown has not finished by then, Delete kills the
  session as it does today and logs that it did.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Sharing the machine with other agents; Agent scratch space), `plans/AGENTS.md` (Executing),
`review_feedback_queue/AGENTS.md` (Lifecycle) and `review_feedback_queue/FILTER.md` (its preamble and existing filters'
shape).

**Planner proposals** are the per-item mechanisms below. A fresh-context planning review checked them for unnecessary
scope; its findings are folded in.

## Per-item outline

Line numbers are approximate; find the code by name.

1. **Discard.** Remove the feedback file and its index line, and update the ledger entry. No code, spec or changelog
   change, and no review gate (P2).
2. **Event-feed liveness check postponed by revisions.** In `serve_events` (`crates/farhelm-helm/src/events.rs`), do not
   reset the `idle` timer after a revision write while `awaiting_liveness` is true, so an unanswered keepalive Ping is
   judged on schedule. Update the `IDLE_PING_INTERVAL` doc comment, which currently says a revision write resets the
   idle window. Extend the existing keepalive test to send revisions while a Ping is unanswered and assert the
   subscriber is dropped within the documented bound.
3. **`BUGS.md` entry for idle input clients.** Add an entry in that file's style per the ledger's Completion criteria:
   what a user can notice, the mechanics in brief, how sure we are (the reviewer's measurements, not reproduced), and
   why it is not being fixed. Documentation only.
4. **`FILTER.md` filter.** Add a separate filter (do not fold it into "Rare, self-correcting glitches and imprecise
   diagnostics": that would weaken that filter's exclusion for durably recorded wrong state). Follow the existing
   filters' shape: an "Added 2026-10-01." line, "A finding matches when both of these hold", and "It does not match when
   the consequence includes any of the following", with the exclusions the ledger lists. Keep the file's preamble
   intact: a filter only decides what is worth triage, never what is acceptable in code.
5. **Duplicate hosts, simplified.**
   - Spec: replace the "shown once" sentence in SPEC.md's host registry paragraph with the rule above. Update
     SPEC_impl.md wherever it describes the duplicate state's 45-second re-check or cadence (the host-state list and the
     cadence paragraph near the `REPROBE_INTERVAL` discussion).
   - Classification: in `record_first_contact` (`crates/farhelm-helm/src/store.rs`), resolve a claimant (`claimant_of`)
     before comparing with the recorded identity and return `Collision { owner }`. Correct the
     `HostStoreError::IdentityClaimed` doc, which says the host re-renders as a duplicate.
   - Remove the automatic re-check: delete the duplicate re-check block at the top of the actor loop in
     `crates/farhelm-helm/src/manager.rs` and the `twin_holding` helper it alone uses, so `Duplicate` waits for a nudge
     with no timer, the way `IdentityMismatch` does. Update the `REPROBE_INTERVAL` doc and the `Duplicate` state's doc.
     Keep the retarget guard that runs when the duplicate state is first published, its test gate, and the test
     `retargeting_during_duplicate_attempt_drops_the_stale_freeze`; only the re-check and its own tests go. Rewrite the
     test that covered automatic recovery into the ledger's "Retry clears the freeze after the other entry is removed"
     test.
   - Retry needs no UI work: the host menu already offers Retry in every state, and the helm's retry nudges the actor
     whatever its state.
   - Wording: change the duplicate detail and remedy text in `crates/farhelm-ui/src/hosts.rs` to name the other entry by
     its display name, looked up in the host list the UI already holds (no API change), and to say: remove that entry or
     change this one's destination, then press Retry. Change the helm's session-refusal text for a duplicate host in
     `crates/farhelm-helm/src/sessions.rs` to match.
   - Tests for both triggers in the ledger (retargeting an entry onto another entry's machine; a re-added reinstalled
     host while the old entry still points at it), plus the Retry test above.
6. **Delete waits for the sink before killing the session.** Last in the stack because it relies on the clarification of
   SPEC.md "Waiting between operations on one host" that `plans/triage-signin-status-paths.md` writes (its item 14:
   terminal I/O may wait on brief, bounded local work during Delete, never on long operations or kill grace periods).
   - Before starting, check that SPEC.md at the stack tip contains that clarification. If it does not (that plan has not
     built item 14, or blocked before it), block this item with the question, per Unattended fallback; PRs 1 to 5 are
     unaffected.
   - Mechanism: in Delete's teardown (`crates/farhelm-supervisor/src/service/teardown.rs`), after the forwarders are
     joined and the last sink reference is dropped, look up the session's sink registry entry
     (`crates/farhelm-supervisor/src/service/terminals.rs`). Only when it is `Reaping`, await its reap receiver with a
     deadline of about 5 seconds (P3), copying the way `ensure_session_sink` already waits on a reaping entry; for a
     live, failed or absent entry, go on at once. A live entry means an attach that raced the delete still holds the
     sink, and waiting on it would burn the whole deadline. On timeout, kill the session as today and log a warning that
     the orderly sink shutdown had not finished.
   - Delete holds the host-wide attachments lock through this wait, which every keystroke, attach, detach and resize on
     the host goes through. That is acceptable under the clarified spec because the normal wait is one tmux round trip
     (the same kind of work Delete already does under the lock), and the full deadline is reached only when tmux itself
     is not answering, in which case terminal I/O is already stalled. Say so in a comment at the wait and in the PR
     description. If implementation shows the normal path is more than brief local work, treat that as a question.
   - The other `kill_session` callers in `crates/farhelm-supervisor/src/service/core.rs` only kill brand-new sessions
     that have no sink, so the ledger's "if a restart path that kills and recreates a session under the same name turns
     out to exist" condition does not apply; no change there.
   - Test: a deterministic check that once Delete returns successfully, the session's sink registry entry is gone, plus
     ordering with the existing fake sink in the teardown tests (gated, if needed) showing the kill comes after the
     reap. Do not try to reproduce the reviewer's timing race.
   - This PR marks this plan's `plans/INDEX.md` line `[executed]`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-feed-sink-identity-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
and `TRIAGE_OUTCOMES.md` Execution fields on each PR) rather than starting over. If it does not exist, this is a fresh
start.

### Galaxy-brain, no-workhorse

The user requires you to use `$scode-galaxy-brain` to achieve this entire goal. Invoke it immediately after setting up
the resume protocol and keep it active for the whole run. The user forbids delegating any unit of your own
decomposition, read-only or writing: you do all of that work yourself and do not ask routing about it, and this demand
overrides galaxy-brain's own judgment of what is worth delegating. The spawns this file calls for (the review gate and
the scope reassessment review) are still routed and launched through galaxy-brain.

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

- Use the `jjstack` skill. The stack's base is not main but the tip of the plan stack, set up per `plans/AGENTS.md`
  (Executing, step 4). PRs already in the plan stack, from earlier plans or an earlier blocked run of this one, are the
  base and are not rewritten.
- One outcome per commit, bookmark and draft PR, in the order listed in The goal. Never combine two outcomes, and never
  split the fix spec+code outcome across PRs. Within this run, if a PR needs correcting, restructure it rather than
  stacking a correction on top.
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect: `fix:` for items 2,
  5 and 6; `docs:` for items 1, 3 and 4. Every `fix:` PR adds a changelog fragment under `releasing/changelog.d/` in the
  same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing the plan stack is the user's job.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- Docs-only PRs (1, 3, 4): `dprint check` on the changed files, nothing else.
- PR 2: `cargo fmt --all -- --check`, clippy on `farhelm-helm`, and a focused nextest selection for the event-feed
  tests.
- PR 5: the same for `farhelm-helm` (store and manager host-state tests), plus `cargo check -p farhelm-ui` for the
  wording change and the UI's host-phase tests if any assert the old text.
- PR 6: fmt, clippy on `farhelm-supervisor`, and a focused nextest selection for the teardown and sink tests, with the
  pinned tmux on PATH.
- Any PR that changes Rust tests or their fixtures: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.

### Review gate

Before finishing each PR other than 1, use the active galaxy-brain skill to delegate a review of that PR's changes. The
user demands this reviewer: a fresh-context agent on Opus 5.5 at high effort, with no review swarm (P1). The prompt
carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md` entry and its item above.
For a PR that changes tests or fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires. Address what the reviewer finds before moving on. Do not write a launch command here or in the
log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new subsystem, registry, recovery protocol,
compatibility layer, or anything else the ledger entry did not imply; these are examples, not a blacklist), and whenever
the same component has needed repeated corrective review rounds, run a fresh-context review through galaxy-brain with
this charter, supplying the ledger entry, the user decisions above, this outline, the current diff and the proposed
departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the filter wording in item 4, the duplicate-message wording in item 5, and the
wait's placement and deadline in item 6. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback (P3 is the agreed one for item 6's wait) or the user's decision; a
review finding or a log entry is not authorization. If an item needs such a decision, record the concrete tradeoff, and
block per `plans/AGENTS.md` (Executing, step 7). Because the PRs form one linear stack, later items sit on top of the
blocked one: finish the PRs before it, record the question, and close the plan as blocked rather than building past it.
If current code or specs have moved so that a recorded decision no longer applies, that is a question for the user too,
per root `AGENTS.md` (Execute triage outcomes); never re-triage an item yourself.

## Done criterion

The plan is complete when all 6 draft PRs exist as one linear stack on the plan stack's tip, each satisfies its ledger
entry's Completion criteria, each that requires it has passed the review gate, each has updated its own
`TRIAGE_OUTCOMES.md` Execution field and removed its queue item, and PR 6 has marked this plan's `plans/INDEX.md` line
`[executed]`. Open, not merged: merging is the user's job. Then close the plan per `plans/AGENTS.md` (Executing, step
8): write its report, write a closing entry in its log, and stop the watchdog.
