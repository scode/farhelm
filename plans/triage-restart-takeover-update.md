# Execute the fourth 2026-10-01 triage batch: the high-priority review queue

Written against main at 5107d37 on 2026-10-01. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

Carry out the 19 triage outcomes recorded in root `TRIAGE_OUTCOMES.md` whose Execution field reads
`` planned in `plans/triage-restart-takeover-update.md` ``, exactly as root `AGENTS.md` section "Execute triage
outcomes" prescribes: one reviewable commit, one stable bookmark and one draft PR per outcome, in a single linear stack.
There is one agreed exception: the user decided that the two terminal-takeover outcomes share one PR, because they are
one code change. That makes 18 PRs, in ledger order:

1. `codex-resume-template-duplicates-selector.md` (fix spec+code)
2. `pi-resume-selector-option-boundaries.md` (discard)
3. `provisioning-child-output-drain-deadline.md` (other: a `BUGS.md` entry, no code)
4. `clipboard-writes-unbounded-blocking-admission.md` (fix spec+code)
5. `session-view-leaks-page-lock.md` (fix code)
6. `row-menu-drifts-on-row-height-change.md` (fix code)
7. `uploads-aborted-silently-on-remount.md` (fix code)
8. `replace-drop-skips-source-delete.md` (fix code)
9. `restart-can-still-deselect-session.md` (fix code)
10. `create-dialog-empty-catalog-refuses.md` (fix code)
11. `probe-drops-add-busy-claim.md` (fix code)
12. `retarget-race-republishes-old-client.md` (fix code)
13. `probe-register-not-helm-owned.md` (fix code)
14. `desktop-start-fails-on-skewed-supervisor.md` (fix code)
15. `terminal-tombstone-never-buried.md` (fix code)
16. `checkout-retry-raw-device-check.md` (fix spec+code)
17. `takeover-latch-misses-attaching-tabs.md` and `new-tab-mount-displaces-owner-during-recovery.md` (fix code, one PR)
18. `update-silently-downgrades-newer-hosts.md` (fix spec+code)

Each outcome's `TRIAGE_OUTCOMES.md` entry (Assessment, Decision, Completion criteria) is the authoritative statement of
what that PR must do. Read the entry and its feedback file under `review_feedback_queue/` before starting the item. This
file adds the plan-time decisions and the mechanisms proposed to satisfy those entries; where the two seem to disagree,
the ledger entry wins and the disagreement is a DECISION to log (or a question, per Unattended fallback).

Acceptance criteria:

- 18 draft PRs exist, stacked in the order above on top of the plan stack's tip (see PR discipline).
- Each PR does what its ledger entry's (or entries') Completion criteria say, as refined by the plan-time decisions
  below, removes its feedback file(s) and line(s) in `review_feedback_queue/INDEX.md`, and updates its own
  `TRIAGE_OUTCOMES.md` Execution field(s) to `complete` with the jj change ID, bookmark and PR URL (record the change ID
  and bookmark before creating the PR, then add the URL to the same change and push again; no separate bookkeeping PR).
- Every PR has passed the review gate.
- PR 18 also changes this plan's line in `plans/INDEX.md` to `[executed]`. Do not delete this plan file.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request:** "use the planning system to queue up execution of everything we just triaged".

**The user's triage decisions (2026-10-01),** recorded in full in each ledger entry's Decision field. The ones that
shape the work most:

- Item 1: refuse derivation the way Grok already does, and add the Codex case as an example to TODO.md's `Near term`
  entry "Re-examine and simplify how launches are represented".
- Item 3: do not fix; record a known issue in `BUGS.md`.
- Item 4: fix only because the fix is very small, and say in SPEC.md that Farhelm assumes the host's clipboard works and
  does not add complexity to support a broken, hung or slow one.
- Item 9: fix at the source (the supervisor keeps a restarting session listed), not by making the UI's deselect more
  tolerant.
- Items 11 and 13: fix because the fixes are tiny.
- Item 16: reuse the identity rule #1200 introduced (inode plus creation time, device number only when no creation time
  was recorded), and make SPEC.md say that is how Farhelm identifies directories and why device numbers are not used.
- Item 17: one shared fix and one PR for both takeover outcomes.
- Item 18: Update never downgrades, and the host list shows that a host runs a newer version than the helm instead of
  merely hiding "needs update" or "old version", so the user can tell something is off.

**The user's plan-time decisions (2026-10-01):**

- P1. Review gate: a fresh-context Opus 5.5 agent at high effort with the general review charter, for all 18 PRs. No
  review swarm.
- P2. Item 8's widening: fix every other helm session route that turns out to be request-task-bound and shares the same
  small mechanism in the same PR; list any that need something different in the plan's final report, without blocking.
- P3. Item 9: while a session restarts, the supervisor keeps listing it with its pre-restart state. No new wire field,
  no UI change.
- P4. Item 18's label is "too new", kept short for the host list's layout. Hovering it shows the full information: the
  helm's version and what the helm knows about the host from its supervisor, such as the host's version and both
  protocol versions.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:** root `AGENTS.md` (Conventional Commits; Finishing work; Execute triage outcomes;
Releases and the changelog; Harness-specific code; Sharing the machine with other agents; Agent scratch space; The live
install is off-limits), `plans/AGENTS.md` (Executing), `review_feedback_queue/AGENTS.md` (Lifecycle), and
`.agents/test-authoring.md` for test changes.

**Planner proposals** are the per-item mechanisms below. A fresh-context planning review checked them for unnecessary
scope; its findings are folded in.

## Per-item outline

Line numbers drift; find the code by name.

1. **Codex resume selector.** Make Codex's `ambiguous_derived_resume`
   (`crates/farhelm-supervisor/src/agent_kind/mod.rs`) refuse a derived template when any argument after the program is
   exactly `resume` or `fork`, the way Grok's check (`grok_has_ambiguous_resume_shape`) scans every argument, with a new
   `SnapshotError` variant worded like `GrokAmbiguousResumeBoundary`. Scanning every argument also catches
   `codex --yolo
   resume <id>`; the cost, a launch whose prompt is literally the single word `resume` being refused,
   is the same tradeoff Grok accepts, and an explicit resume template still bypasses the check
   (`IntegrationSnapshot::resolve` already skips it for an override). Read root `AGENTS.md` "Harness-specific code"
   first; this belongs in Codex's integration impl, not in shared code. Tests: `codex resume <id>` and
   `codex --yolo resume <id>` without a template refuse, with an explicit template resolve, and a plain `codex` launch
   still derives. Spec: SPEC.md's derived-resume paragraph (around "Grok instead refuses to derive a template …") names
   Codex beside Grok. TODO.md: add the Codex case as a second example to the `Near term` entry "Re-examine and simplify
   how launches are represented", in the style of the existing `yolo-guard-skips-resume-template.md` example.
2. **Pi discard.** Remove the feedback file and its index line; no code or spec change.
3. **`BUGS.md` entry.** Add an entry in that file's style (what the user sees, the mechanics in a short TLDR, how sure
   we are, why we are not fixing it), using the ledger entry's Completion criteria as the content. Keep the paths, hosts
   and local details out; name the OpenSSH version checked. Remove the feedback file and its index line.
4. **Clipboard cap.** In `post_clipboard` (`crates/farhelm-helm/src/clipboard.rs`), take an owned permit from a small
   fixed-size semaphore with `try_acquire_owned` before `spawn_blocking`, move the permit into the blocking closure, and
   return the existing 204 without writing when none is free. A handful of permits is plenty (the webview keeps only a
   few requests in flight); pick one and say why in its doc comment. The semaphore lives in the helm's shared state
   (`AppState` in `crates/farhelm-helm/src/lib.rs`, constructed there and in `crates/farhelm-helm/src/rest_harness.rs`),
   not in a process-wide static, so parallel tests in one process cannot starve each other. Log a dropped write at most
   once per stuck episode, not per request. Update the module docs' "Caps" section. Test: a sink that blocks on a
   channel; writes beyond the cap return 204 without reaching the sink; after unblocking, a new write reaches it. Spec:
   the clipboard best-effort sentence in SPEC.md's Terminal experience section gains the assumption the ledger states.
5. **Page lock.** In `crates/farhelm-ui/src/session_view.rs`, move header Restart, "Restart with" (including its
   failed-attempt state) and the interrupted card's Restart onto the self-releasing guard #1153 gave the Replace sites
   (see `ops.rs` and the confirmation slot), or make the view release its claim on unmount; pick whichever is smaller
   and log it. #1153's notes explain why it left the Restart sites alone; read them before choosing. Clear the
   interrupted card's Replace prompt, and its claim, when the card stops rendering. Tests: the ledger requires all four
   cases (unmount while a Restart confirmation holds the claim, while an open "Restart with" dialog holds it, during an
   in-flight Restart, and the hidden interrupted-card Replace prompt). Use UI unit tests where the guard is unit
   testable; otherwise browser regressions that share one helper (hold the claim, delete the session from another
   client, check that the sidebar still opens another session).
6. **Row menu.** Extend the reorder check (`rows::menu_row_reordered`, `crates/farhelm-ui/src/rows.rs`, used by
   `commit_listing` in `crates/farhelm-ui/src/list/view.rs`) so it also reports a move when a row above the open one
   gains or loses its detail line, and update the comment above the close effect that accepts this residual. Use
   whatever the row already computes for "has a detail line" (`list/row.rs`) rather than measuring heights. The host
   row: include it only if the same mechanism covers it cheaply, else log the DECISION. Unit tests beside the existing
   ones.
7. **Uploads across remounts.** In `crates/farhelm-ui/assets/terminal.js`, keep each pane's upload status messages on
   the pane's status element itself (for example a property or dataset on it), which already lives exactly as long as
   the pane and outlives remounts, and have the next mount repaint them. Do not keep a page-wide map keyed by element
   id: it would need cleanup when a tab closes and would repaint stale messages when the session is reopened. On
   `dispose()`, turn each upload still in flight into a visible "upload of X was interrupted" message: an upload stopped
   while the file was still being read was definitely not published; one stopped after its bytes were sent has an
   unknown outcome and the message says it may have been published (SPEC.md Attachments: the failure response must
   distinguish the two). Stale "attaching…" for an upload that is no longer running still clears. Tests: the JS unit
   harness if the logic can be exercised there, otherwise a browser regression.
8. **Replace on a helm-owned task.** Wrap the body of `replace_session` (`crates/farhelm-helm/src/sessions.rs`) in
   `run_owned` (`crates/farhelm-helm/src/lib.rs`), as the host routes in `hosts.rs` do. Then check the other session
   routes in `sessions.rs` against SPEC_impl.md "Who owns an accepted action" and apply P2. Regression: a request
   dropped after the create was sent still deletes the source; the helper's existing test shows how to drop a request.
9. **Restart keeps the session listed.** In the restart path in `crates/farhelm-supervisor/src/service/core.rs` (the
   "Off the map for the duration" block), keep removing the session from the session map exactly as today, so stop,
   delete and attachment installs stay excluded. Before removing it, capture its pre-restart listing row and hold it in
   a small side map for the restart window, cleared on the three existing ways out of the window (the republication in
   `publish_relaunched`, the restore after a failure, and the no-restore branch for a session deleted meanwhile). The
   listing (`list_all` in `crates/farhelm-supervisor/src/service/listing.rs`) merges those held rows into its reply as
   they are and never runs its per-session pass on them: that pass reads the tmux pane, launch sentinels and exit state,
   and on a pane mid-respawn it would report the session exited or record an exit, the outcome the existing comment
   calls worse than listing nothing (P3). Say in the code where the held row's status comes from (the row as last built
   before the restart began, not a fresh pane read). Update the comment that accepts the omission. Regression: a listing
   forced into the relaunch window includes the session with its pre-restart state. Use the supervisor's existing test
   hook facility (`fault_hooks!` / `FaultHooks` in `core.rs`; the existing `replacement_fault` hook may already serve as
   the pause point); adding one entry to it is the normal pattern, not a new subsystem, and needs no scope reassessment.
10. **Create dialog catalog.** In `crates/farhelm-ui/src/list/create_form.rs`, a failed or pending catalog read must not
    mark a selection incompatible; mirror `restart_with.rs`, which already treats a failed read as compatible, and fix
    its pending case too so both share one rule. Show the read error with a retry in the create dialog. Unit tests where
    the compatibility logic lives (`launch_composer.rs`).
11. **ADD busy claim.** Remove the unconditional busy-marker removal in `resolve_failed_add_discovery`
    (`crates/farhelm-helm/src/provisioning/service.rs`); every failure path already releases its own claim. Check each
    caller to confirm none relied on that removal. Test: a probe while an ADD holds the claim leaves the host busy.
12. **Retarget race.** In `publish_refresh` (`crates/farhelm-helm/src/manager.rs`), write the refreshed connection only
    if the published one is still this actor's own, decided inside the same `send_modify` the retarget's withdrawal
    uses, so the check and the write are atomic. Deterministic regression through a gate seam like the existing
    `DuplicatePublicationGate`, landing a retarget between the refresh's check and its publish.
13. **Probe registration.** Run the probe's post-discovery work (`register` and `resolve_failed_add_discovery` in
    `provisioning/service.rs`, reached from `probe_host` in `provisioning/http.rs`) through `run_owned`, as #1196 did
    for the other host edits, and correct the route comment in `lib.rs` that calls the probe non-mutating. Regression: a
    probe request dropped after the save still leaves the host registered and dialed.
14. **Desktop startup on a skewed supervisor.** In the startup wait in `crates/farhelm-ui/src/desktop.rs`, stop as soon
    as the local host's phase is a version skew, identity mismatch or unverified identity, and fail with that state's
    details plus an instruction to stop the supervisor the user started; drop "managed" from the message when the app
    spawned nothing. Unit test beside the existing timeout-text test.
15. **Tombstones.** Add the tombstones map to the departure loop in `sync()` (`crates/farhelm-ui/assets/terminal.js`),
    with `held` falling back to `tombstones.get(el)` so a departed tombstone is handled without a throw, and bury it
    (dispose its xterm). Check that a returning terminal then mounts or shows its Detached notice. Test in the JS
    harness if reachable, else a browser regression.
16. **Directory identity in the launcher.** Once `verify_identity` has accepted the folder on a retried create, hand the
    launcher's preparation input (`core.rs`, where `path_identity` is passed to the launch) the folder's identity as
    just observed, instead of the one stored at allocation. The launcher's plain comparison
    (`crates/farhelm-supervisor/src/launch.rs`) then compares two readings taken seconds apart within one create, which
    no remount can separate, and the remount-tolerant rule stays defined in one place
    (`working_copies::same_directory`). Its doc comment must say why this comparison is not the kind of device-number
    identity check the new SPEC sentence forbids. If that turns out not to work, carry the recorded birth time into the
    launcher and call the shared rule instead, and log the DECISION. Regression for a retried create after a
    device-number change; the #1200 tests show how device numbers are faked. Spec: the managed-checkout section of
    SPEC.md (beside "durable identity capture") states the identity rule and its reason, per the ledger's Completion
    criteria.
17. **Takeover-safe tab mounts.** In `crates/farhelm-ui/assets/terminal.js`, after a session view's initial open, mount
    every newly seen tab on the attach route that is refused when another client holds the session; only opening the
    session and an explicit take-control or reconnect action use the displacing route. A refusal lands in the normal
    latched "Detached … take control" state, including for a pending mount the latch cancels. Browser regressions on
    Chromium and WebKit using the existing two-client takeover setup in `e2e/`: a tab attaching during another window's
    takeover, and a view recovering from a dropped connection that sees a tab the other client created.
18. **No downgrades, and "too new".**
    - Helm: refuse Update (planning and revalidation in `provisioning/service.rs`) when the host's probed build is newer
      than the helm's, with a clear error naming both versions; the existing "host older than me" comparison behind the
      `old version` advisory (SPEC_impl.md) shows where to compute it.
    - Wire shape, no new host phase: a new phase name would spread through the phase tokens shared by logs, JSON and the
      UI, the status colours and the contract tests in two crates. For a connected host, add one flag beside the
      existing `old_version` flag, computed by a newer-than comparison next to the older-than one. For a version-skewed
      host, both protocol versions and both builds already reach the UI, so it shows "too new" when the host's protocol
      is the higher one without a wire change. The hover needs no new field either: a connected host shares the helm's
      protocol, and the UI already knows the helm's build.
    - UI (`crates/farhelm-ui/src/hosts.rs`, `provisioning.rs`): hide Update on such rows and skip them in "update all";
      show the label "too new" instead of "needs update" or "old version" (P4), and a hover with the helm's version and
      the host's version and both protocol versions, as far as the helm knows them. Extend `phase_display_label` and its
      tests. The remedy hint under a skewed row ("update the farhelm binary on {host} (or this helm)") must not
      recommend Update on a "too new" row: make it depend on which side is newer.
    - Spec: SPEC.md's Update authorization text says Update never downgrades a host, and the host version advisories say
      a host newer than the helm is shown as "too new".
    - Tests: the helm refusal, the "update all" skip, and the label, hover and hint text.
    - This PR marks this plan's `plans/INDEX.md` line `[executed]`.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-triage-restart-takeover-update-log.md` in the parent directory of the checkout you run in, derived as that
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
- One outcome per commit, bookmark and draft PR, in the order listed in The goal, except PR 17, which carries both
  takeover outcomes by the user's decision. Never combine other outcomes, and never split a fix spec+code outcome across
  PRs. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top.
- Commit messages and PR titles use Conventional Commits. The type reflects the user-visible effect: `fix:` for items 1,
  4 to 18; `docs:` for items 2 and 3 (queue bookkeeping and a `BUGS.md` entry). Every `fix:` PR adds a changelog
  fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); use
  `kind: none` with a reason where nothing changes for a user; validate with
  `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- PRs stay drafts. Never mark one ready and never merge; landing the plan stack is the user's job.

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression in each PR, through
`scripts/record-test-run.py` for Rust test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist:

- PRs 2 and 3: `dprint check` on the changed files, nothing else.
- Rust PRs (1, 4, 8, 9, 11, 12, 13, 16, 18): `cargo fmt --all -- --check`, clippy on the touched crate, and focused
  nextest selections for the touched module's tests and the new regression. PR 9 and PR 16 also need the supervisor
  tests around restart and checkout creation with the pinned tmux on PATH.
- UI Rust PRs (5, 6, 10, 14, 18): `cargo check -p farhelm-ui` and the UI crate's unit tests touching those views; PR 14
  also `cargo check -p farhelm-ui --features desktop` and the desktop nextest selection for the new test.
- JS PRs (7, 15, 17): `cd crates/farhelm-ui/js-tests && node --test` when the logic is covered there, and the new and
  nearest existing Playwright specs on Chromium and WebKit through the recorder, after the builds root `AGENTS.md`
  names. PR 5 too if it adds a browser regression.
- Any PR that changes Rust or browser tests or their helpers: run `python -B scripts/check-test-sleeps.py` per
  `docs/test-sleep-check.md`, and apply `.agents/test-authoring.md`.

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of that PR's changes. The user demands
this reviewer: a fresh-context agent on Opus 5.5 at high effort, with no review swarm (P1). The prompt carries the full
charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria for that PR: its `TRIAGE_OUTCOMES.md` entry (both entries for PR
17), its item above, and the plan-time decisions that apply to it. For a PR that changes tests or fixtures, include the
full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Address what the reviewer finds before
moving on. Do not write a launch command here or in the log; the harness-shellout skill owns launch mechanics.

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
alternatives considered: in particular the clipboard cap's size (item 4), guard versus unmount release (item 5), whether
the host row menu was covered (item 6), which other session routes item 8 fixed or reported (P2), how item 9 keeps the
session listed, how item 16 shares the identity rule, how item 18's hover and hint read, and every spec wording change.
The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the ledger's requirements still hold. A material scope expansion, a weakened guarantee, or an
omitted required behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not
authorization. P2 is an agreed fallback for item 8: report, do not block. If an item needs such a decision, record the
concrete tradeoff, and block per `plans/AGENTS.md` (Executing, step 7). Because the PRs form one linear stack, later
items sit on top of the blocked one: finish the PRs before it, record the question, and close the plan as blocked rather
than building past it. If current code or specs have moved so that a recorded decision no longer applies, that is a
question for the user too, per root `AGENTS.md` (Execute triage outcomes); never re-triage an item yourself.

## Done criterion

The plan is complete when all 18 draft PRs exist as one linear stack on the plan stack's tip, each satisfies its ledger
entry's Completion criteria as refined by the plan-time decisions, each has passed the review gate, each has updated its
own `TRIAGE_OUTCOMES.md` Execution field(s) and removed its queue item(s), and PR 18 has marked this plan's
`plans/INDEX.md` line `[executed]`. Open, not merged: merging is the user's job. Then close the plan per
`plans/AGENTS.md` (Executing, step 8): write its report, write a closing entry in its log, and stop the watchdog.
