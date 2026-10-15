# Show when turning session confirmations back on fails to save

Written against main at 93092359 on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where the outcome below tells you to change them.

This plan has no dependency on another plan.

## The goal

Settings has a "skip session confirmations" switch. With it on, Delete, Replace, Restart and closing a terminal tab act
on the first click. The choice is a helm-side preference, and SPEC.md (Errors and diagnostics) lists it among the
preferences whose save failures are logged but stay silent. A review finding showed the dangerous case: a user turns the
switch off again to get confirmations back, that save fails, nothing is shown, and the next reload or app restart brings
the stored opt-out back, so destructive actions are first-click again without the user knowing. The maintainer decided:
"show error". Carry out this triage outcome, recorded under its heading in `TRIAGE_OUTCOMES.md` (read it in full first):

- `confirmation-restoration-failure-reloads-optout.md` (`fix code`)

### Acceptance criteria

- When the save that turns confirmations back on (switching "skip session confirmations" off) fails, Settings shows an
  error next to the switch, in the style other Settings save errors use, saying that confirmations may still be off for
  this helm and to try again. The switch's local state stays as the user set it for the current page, as today.
- A later successful save of that switch clears the error. A save superseded by a newer choice before it settled is not
  a failure to report: the existing completion channel reports a superseded write the same way as a failed one, so tag
  each restoring click (a per-click generation counter in the component is enough) and show the error only when the
  failed write is still the newest click on that switch. Turning the switch off, on and off again must not show an error
  left over from an earlier click.
- Turning the opt-out on (skip confirmations) keeps today's silent behavior: losing that convenience is safe.
- SPEC.md (Errors and diagnostics) no longer lists the session-confirmation opt-out among the silent preference failures
  without qualification: it says that a failed save restoring confirmations is shown. Check SPEC_impl.md for a matching
  statement and keep the two consistent.
- A browser test covers it: fail the preferences write (the specs already intercept `**/api/preferences` with
  `page.route`, for example `e2e/tests/welcome.spec.ts`), switch "skip session confirmations" off, and assert the error
  appears; then stop failing the write, switch it on and off again, and assert the error is gone and the helm stores
  confirmations as on. `e2e/tests/session-confirmations.spec.ts` is the natural home.
- A changelog fragment (`kind: fixed`): when turning session confirmations back on fails to save, Settings now says so.
- The feedback file and its index entry are removed in the same PR, with the Execution field updated.

## Requirement sources

**The user's decisions (2026-10-14):** "show error". Review gate: a single fresh-context gpt-6.1-sol reviewer at high
effort, no swarm. No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** root `AGENTS.md` "Execute triage outcomes" and its changelog rule; SPEC.md "Errors
and diagnostics" (every failed operation surfaces an actionable error, with the listed best-effort exceptions); the
per-field preference write queue in `crates/farhelm-ui/src/api.rs` and its documented ordering contract.

**Planner proposals:** reporting only the restoring direction, the wording, using the existing completion channel.

## Outline

- `crates/farhelm-ui/src/settings.rs`: the switch's `onchange` calls `store_preference` with
  `PreferenceValue::SessionConfirmations(..)`, which is fire and forget. `crates/farhelm-ui/src/api.rs` already has a
  per-write completion channel (`PreferenceCompletion`, used by `write_template_binding`) that reports whether the
  field's newest choice was acknowledged. For the restoring direction, write through that channel (generalize
  `write_template_binding`'s field assertion, or add a small sibling for this field) and show the error when it reports
  failure. Check how the channel reports a superseded write so a newer choice does not produce a stale error.
- One PR, `fix:`, with the SPEC.md narrowing, the browser test, the changelog fragment and the queue bookkeeping.

### Validation

Per root `AGENTS.md` "Finishing work", choose the smallest set that covers the change: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the `farhelm-ui` unit tests near the preference queue if you change it
(through the recorder), the changed spec and its neighbors on Chromium and WebKit through the recorder (builds first, as
root `AGENTS.md` describes), the test-sleep checker since a browser test changes, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

Examples of departures that need the scope reassessment: reporting failures for other preferences, retrying writes
automatically, or blocking the switch until the save settles.

Decisions to log: the exact wording and placement of the error, how supersession is handled, the SPEC.md wording.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-confirmations-save-error-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md`
(Executing one plan, step 7) before any work.

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
a notification when any of these happens:

- a watched filesystem drops under 10% or under 5 GB free, or available memory drops under 10%, whichever comes first;
- the number keeps falling;
- the condition recovers;
- the monitor itself fails.

In Claude Code a notification is a stdout line of a monitor started with the Monitor tool; elsewhere, the harness's
equivalent. A file update alone is not a notification.

Before relying on the watchdog:

- verify delivery with a harmless synthetic alert;
- verify failure detection by killing a throwaway monitor and confirming you are told;
- if you omit periodic heartbeat checks, also stall a throwaway monitor without killing it and confirm you are notified
  within two sample intervals, since a monitor cannot detect its own sampling loop hanging.

If any of these notifications is unavailable or unverified, say so in the log and check the status file at least once a
minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor, or a
heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching work;
2. remove build output and scratch you own;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- Follow root `AGENTS.md` "Execute triage outcomes": one reviewable commit, stable bookmark and draft PR per outcome, in
  a single linear stack in the order given under Outline above. Each PR carries its outcome's agreed change, the removal
  of its feedback file and `review_feedback_queue/INDEX.md` entry, and its `TRIAGE_OUTCOMES.md` Execution update. Record
  the jj change ID and bookmark in that Execution field before creating the PR, then add the PR's URL (with `/changes`
  appended, per the user's PR-link rule) to the same change and push it again; no separate bookkeeping PR. Replace the
  Execution field's current `planned in ...` text rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/confirmations-save-error/<nn>-<short-name>` (for example
  `plan/confirmations-save-error/01-confirmations-error`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `fix`, with a changelog fragment (`kind: fixed`) in the
  same commit, per root `AGENTS.md`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the outcome's full
  `TRIAGE_OUTCOMES.md` entry.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

PRs in this stack that only remove feedback files and update the ledger, and change nothing else, may share one review
run that covers all of them together, instead of one run each.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist);
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above. If current code or specs invalidate a recorded triage decision, that is a block: root `AGENTS.md`
(Execute triage outcomes) returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per outcome listed under The goal unless the
acceptance criteria above say otherwise, that together meets the acceptance criteria above, each PR passed its review
gate, and each outcome's `TRIAGE_OUTCOMES.md` Execution field names its change ID, bookmark and PR. Open, not merged. If
a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md`
(Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
