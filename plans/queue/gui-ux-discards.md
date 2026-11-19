# Fifty discards from the 2026-11-18 GUI UX triage

Written against main at e3068e2bd on 2026-11-18. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

Execute fifty `discard` outcomes recorded on 2026-11-18, per `review_feedback_queue/.agents/execute-outcomes.md`,
"Execute triage outcomes", with one agreed exception: all fifty go in a single PR (see Decisions). Their
`TRIAGE_OUTCOMES.md` entries are the binding statement of what to do; read each from `main@origin`:

1. `held-start-test-lacks-prelaunch-fencing-witness.md`: Restart test doesn't check the held start ends before the new
   launch.
2. `rebuild-order-test-misses-persistence-boundary.md`: Rebuild test doesn't check the new snapshot is saved before the
   old is deleted.
3. `closed-stdin-epipe-witness.md`: The fake-ssh test never proves the failed write happened.
4. `hook-waiter-missing-invocation-binding.md`: A repeated hook callback can be credited with an earlier one's clean
   finish.
5. `destructive-action-tests-omit-request-targets.md`: Destructive-action tests don't check which session/host the
   request targets.
6. `signal-test-child-lacks-unwind-cleanup.md`: Test processes outlive a failed test because cleanup is armed late.
7. `failed-client-close-test-lacks-writer-premise.md`: "Endless sender" test can run before the sender starts.
8. `staged-start-test-lacks-reader-drain-witness.md`: "Nothing reaches a terminal before its owner is recorded" test can
   miss early bytes.
9. `peak-memory-test-lacks-baseline-headroom.md`: Snapshot memory test could hide a spike under an earlier peak.
10. `malformed-snapshot-fixtures-mask-intended-validators.md`: Corrupt-snapshot fixtures are refused by a size check
    before the check they name.
11. `trailing-removed-live-alternate-screen-reattach-oracle.md`: No test checks reconnecting to a live full-screen
    program shows its screen.
12. `desktop-state-rewrite-blocks-startup.md`: A bad desktop settings file stops the app at a startup alert.
13. `attention-list-refresh-failure-scope-unresolved.md`: A host whose session list stops refreshing gets no attention
    row.
14. `failed-setup-refresh-overwrites-held-lifecycle-status.md`: After a failed registry sync, a refresh shows Connecting
    over a running Suspend.
15. `notification-read-failure-leaves-stale-cell.md`: A failed notification reload leaves the bell wrong until the next
    event.
16. `attention-save-error-can-exhaust-list-height.md`: A save error plus a wrapped header leaves no room for attention
    rows.
17. `short-progress-card-can-hide-overflow-notice.md`: A very short progress card clips "Full details in Hosts".
18. `open-source-notices-window-failure-is-silent.md`: Open-Source Notices does nothing if its window can't open.
19. `desktop-startup-reopen-can-wait-on-reused-pid.md`: Reopen helper waits forever if the old process number is reused.
20. `open-types-acknowledgement-only-in-dialog.md`: "The helm stores any file-type list without the acknowledgement".
21. `trailing-supplied-timing-comment-defect-is-source-confirmed-and-already-owned-by-g020-l7.md`: Second copy of the
    relay-ordering comment finding.
22. `trailing-shared-rollback-allowance-does-not-remove-the-refusal-tests-observable-boundaries.md`: Stand-in cleanup
    does not hide destructive actions.
23. `trailing-deleting-extends-the-known-incomplete-backend-phase-test-matrix.md`: Host-phase test also misses Deleting.
24. `trailing-the-nothing-decided-terminology-refutation-does-not-cover-the-stronger-no-change-comment.md`: "Nothing was
    decided" wording does not promise nothing changed.
25. `trailing-rest-admission-owns-registration-before-the-launch-task-is-spawned.md`: Dropped client can't strand a
    half-registered environment launch.
26. `trailing-provider-preparation-callbacks-await-insertion-before-reporting-phases.md`: Preparation progress steps are
    not misattributed.
27. `trailing-atomic-install-failed-cleanup-clears-safe-session-limit-refusal.md`: Failed-cleanup path loses the "SSH
    session limit" explanation.
28. `trailing-deletion-ownership-refutation-remains-separate-from-warning-repair.md`: Recipe deletion keeps its hold
    when the client disconnects.
29. `trailing-launch-excerpt-full-row-allocation-before-500-byte-trim.md`: 500-byte launch-error excerpt briefly holds
    more text.
30. `trailing-launch-excerpt-whole-row-intermediate-allocation.md`: Launch-error excerpt memory + earlier-run quoting.
31. `gone-retry-forgets-pinned-foreground-child.md`: A reconnect that finds the host gone forgets the foreground
    program.
32. `normal-quit-misses-empty-download-directory.md`: Quit can leave an empty hidden download staging folder.
33. `numeric-version-guard-hides-artifact-filenames.md`: Filenames starting with a version number get no file actions.
34. `tab-helper-can-fail-after-open-succeeds.md`: A slow tab can report opened, then vanish without the folder error.
35. `spare-ancestry-fallback-skips-destination-recheck.md`: Refused prepared checkout clones into a swapped-in folder.
36. `network-state-wal-failure-topology-unresolved.md`: Database journal mode on a network-mounted state folder.
37. `muse-unmarked-same-process-child-attribution.md`: A Muse sub-agent report might be taken as the session's
    conversation.
38. `refused-add-rewrites-new-destination-owner.md`: Refused host setup can rewrite another busy host's install paths.
39. `desktop-owner-record-lacks-boot-domain.md`: Linux desktop supervisor record can match a different process after
    reboot.
40. `changed-project-key-cleanup-absence-unresolved.md`: After switching to a key for another Tensorlake project,
    cleanup may forget resources.
41. `first-state-directory-ancestor-durability-unresolved.md`: First-run state folder's parents aren't explicitly
    flushed.
42. `ambiguous-codex-seed-renewal.md`: Re-sending a saved Codex login after an uncertain upload can overwrite a renewed
    one.
43. `launcher-live-draft-submission-source-gap.md`: Launcher reads current choices at submit (unresolved).
44. `header-copy-warning-survives-different-copy.md`: Copy warning describes the earlier field for up to 1.5 s.
45. `secondary-button-erases-intercepted-primary-ownership.md`: A second mouse button during a link click stops the link
    from opening.
46. `truncated-empty-filter-result-omits-clear-actions.md`: Empty result from a capped listing has no Show all links.
47. `transfer-result-height-can-hide-destination.md`: A long download result line can be cut off.
48. `backup-warning-can-imply-inconsistent-live-copy.md`: Backup sentence in v0.33.0 release notes reads like a recipe.
49. `long-launch-status-crops-painted-edge.md`: Launch progress pill's edge runs off screen.
50. `subline-progress-preview-hides-diagnosis.md`: Host diagnostic preview can round to zero lines.

### What each PR does

A discard means only "not worth the human's time at this time" (`review_feedback_queue/AGENTS.md`, "Lifecycle"). The one
PR removes each listed feedback file and its `review_feedback_queue/INDEX.md` line, records each outcome's execution,
and moves each ledger entry, with its final Execution field naming the PR, to the end of `TRIAGE_OUTCOMES_ARCHIVE.md`.
No spec or code change, no TODO entry. Entries that call an item a duplicate leave the item it duplicates queued. Type
`chore:`, no changelog fragment.

Each PR removes its feedback file and index line and moves its ledger entry to the archive.

### Notes on individual outcomes

- `peak-memory-test-lacks-baseline-headroom.md`: its Execution record also says the main case was fixed by #3209
  (3aed22760), and only the remainder is discarded.
- The four outcomes whose entries say "say so if you want it kept" (`destructive-action-tests-omit-request-targets.md`,
  `signal-test-child-lacks-unwind-cleanup.md`, `malformed-snapshot-fixtures-mask-intended-validators.md`,
  `trailing-removed-live-alternate-screen-reattach-oracle.md`) are plain discards: the maintainer picked "Apply the rule
  as listed".

### Acceptance criteria

- One PR that removes exactly the fifty listed feedback files and their index lines, and moves exactly their fifty
  ledger entries to the end of the archive with the Execution field naming the PR; nothing else changes.
- `review_feedback_queue/INDEX.md` still lists exactly the feedback files present, and `dprint check` passes on the
  changed files.

## Decisions already made

- **The outcomes:** the maintainer's picks on the 2026-11-18 GUI UX triage page (questions QA to QG, rounds 1 to 5), or
  the clearly-settled rule where an entry says so.
- **Grouping:** the maintainer approved these as one plan on the 2026-11-18 planning page (Q1, seven plans for the day's
  111 outcomes).
- **One PR for all fifty** (the maintainer's pick on the 2026-11-18 planning page, Q2): an agreed exception to one PR
  per outcome. Every outcome still gets its own Execution record and archive move.
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (the maintainer's pick, planning page Q3; see
  Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes (planning page Q4).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **No release note** (the maintainer, 2026-11-18: all of these are "no release note needed"). PRs whose type calls for
  one still leave their changelog fragments.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the `TRIAGE_OUTCOMES.md` entries listed above and the Decisions above.

**Binding repository constraints:** `review_feedback_queue/.agents/execute-outcomes.md` (a discard PR contains only the
queue removal and outcome bookkeeping) and `review_feedback_queue/AGENTS.md`, "Lifecycle"; root `AGENTS.md`,
"Conventional Commits".

**Planner proposals** (change them if the code says otherwise, logged as a DECISION): a small script in the scratch
directory may do the fifty removals and moves mechanically; check its result against the list.

## Outline

One PR:

1. `plan/gui-ux-discards/01-discards`

### Validation

Queue bookkeeping only: `dprint check` on the changed files, and a check that `review_feedback_queue/INDEX.md` lists
exactly the feedback files present. No runtime tests (root `AGENTS.md`, "Tests and validation"). The reviewer checks
that each removal and archive move is exact.

## Fallbacks

- If a feedback file is already gone from main (another change removed it), move only its ledger entry to the archive,
  with an Execution note saying so.
- If a listed outcome's entry is no longer `pending` on main, leave it out of the PR and say so in the report.

Decisions to log: any outcome left out of the PR, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-gui-ux-discards-log.md` in the parent directory of the checkout you run in, derived as that section says,
and the watchdog's status file is `farhelm-plan-gui-ux-discards-watchdog.json` beside it. Resolve both to absolute paths
before you start; this file does not create either. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests on the machine you run on, as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`. The maintainer chose this on 2026-11-18; do not use the boat sandboxes
(`.agents/boat.md`) for this plan.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (one of its remotes
points at that GitHub repository; if not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill
with the log file's absolute path. If the log exists, read it and resume where the previous session left off,
cross-checking it against reality (the jj graph, bookmarks and open PRs) rather than starting over. If it does not
exist, this is a fresh start. A plan that an earlier executor worked on, or that came back from review, also gets the
resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

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
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

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

- One reviewable commit, stable bookmark and draft PR per outcome, in a single linear stack, using the `jjstack` skill,
  as `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes", requires (the outline above names
  any agreed exception). The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/gui-ux-discards/<nn>-<short-name>`, as the outline names them.
- Each PR carries its outcome's agreed change, removes the outcome's feedback file and its
  `review_feedback_queue/INDEX.md` line, and moves the outcome's `TRIAGE_OUTCOMES.md` entry, with its final Execution
  field, to the end of `TRIAGE_OUTCOMES_ARCHIVE.md` (`review_feedback_queue/AGENTS.md`, "Lifecycle"). Record the change
  ID and bookmark in the Execution field before creating the PR, then add the PR's URL to the same change and update the
  PR; never make a separate bookkeeping PR.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"), whatever this plan's Release note section says.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.2-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a
review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline, and the outcome's `TRIAGE_OUTCOMES.md` entry). When a PR changes tests or their fixtures,
include the full text of `.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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
working log with the true value; never edit this plan file. If current code or specs invalidate an outcome's decision
(`review_feedback_queue/.agents/execute-outcomes.md`), that outcome is a block, not something to reinterpret.

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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives, together
meeting the acceptance criteria above, each having passed its review gate, and each having recorded its outcome's
execution and moved its ledger entry to the archive. Open, not merged. If a `## Decisions` section exists, its latest
entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
