# Feedback send as P1 of the release brick test

Written against main at aa0cb0c4 on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file's decisions direct you to change them.

This plan has no dependency on another plan.

## The goal

The release brick test (`releasing/mac-vm-test/BRICK-TEST.md`) gains a **P1** category that runs immediately after P0
succeeds. Its one check sends a real piece of feedback from the candidate app to the live endpoint at farhelm.io,
through the app's own `?` menu and Send feedback dialog. That proves a release can still send feedback, which nothing in
the release gate checks today: the endpoint's handler tests run only in the on-demand CI website job, the browser test
of the feedback dialog intercepts the request (and its CI job is disabled), and the endpoint deploys separately from the
app, so it can change without any app release.

The sent feedback has a fixed first line, `[brick-test] vX.Y.Z` (the candidate's version), so the issue it creates in
the private feedback inbox is titled `Feedback: [brick-test] vX.Y.Z`. The maintainer filters notifications on that
title.

You change the brick-test procedure, its coordinator and its tests. You do not run a brick test: it needs the
maintainer's Mac test host, and this plan only makes the next one include P1.

### Acceptance criteria

- **Where P1 runs.** In the Mac guest of P0's last fixture (`p0-updater-upgrade`), on the candidate app that P0's update
  steps left running there, after P0's five checks have passed and their evidence is recorded, and before that fixture
  is exported and cleaned up. No new clone, no new install.
- **What P1 does.** Through computer use in the guest: open `?` → send feedback; enter the message; leave the contact
  field blank; check that the dialog's display of what is sent with the message shows the candidate's version and the
  desktop app as the surface (it reads `desktop app` today); press Send. **Pass:** the dialog closes by itself after
  Send, with no failure message. That is sufficient: per SPEC.md "Feedback", a failed send says so and keeps the dialog
  open with the typed text, and the dialog closes only on success. The thanks it shows first lasts only 1.5 seconds
  (`THANKS_MS` in `crates/farhelm-ui/src/feedback.rs`), so seeing it is not required. A failure message, or a dialog
  that stays open, fails the check. Describe the dialog in RECIPE.md by function (the message field, the contact field,
  the display of what is sent, Send), not by layout or exact label text: the pending plan `feedback-dialog-polish.md`
  restyles it and renames its labels without changing its behavior.
- **The message.** First line exactly `[brick-test] vX.Y.Z` with the candidate's version. Then a blank line and one
  fixed sentence saying it is an automated release brick test checking that this release can send feedback, and that it
  needs no action. Write the exact sentence into RECIPE.md once and use it verbatim every run. Enter it with the
  verified guest text workaround from root `AGENTS.md` and OPERATIONS.md (guest-only clipboard via the journaled
  controller, then Edit → Paste), and inspect the pasted text before pressing Send.
- **Evidence.** P1 is a GUI-only check (`gui: True, command: False` in the coordinator's per-check flags;
  `add_check --gui` already makes such checks). No command-side observation of a successful send exists: the helm logs
  only failed sends (`crates/farhelm-helm/src/feedback.rs`). Retain a PNG of the filled dialog before Send and one after
  it showing the dialog closed (or the thanks, if caught), with the agent's observation as the detail.
- **Verdicts.** The coordinator reports `p1.verdict` (with the check's name) apart from `p0.verdict`, the way it reports
  P0. The full `verdict` includes P1, since P1 lives in a planned pass; say so in BRICK-TEST.md so "separately" is not
  read as "excluded". Older run manifests report `p1` as `not planned`, mirroring P0's handling of historical manifests.
  A P1 failure is reported to the maintainer promptly, with its own verdict, and does not block the bonus passes, which
  test different things. P1 runs only after a P0 pass; a P0 failure or could-not-run leaves P1 not run.
- **Promotion.** Whether a P1 failure blocks promotion is the maintainer's call each time (maintainer decision). The
  procedure reports P1's verdict and writes no rule about promotion; `releasing/AGENTS.md` step 8 at most names P1 among
  the results the maintainer gets.
- **Stop scopes.** "Stop after P0 checks" keeps its exact meaning: P0 only, no feedback sent, and P0's last fixture is
  still cleaned up normally. A new "stop after P1 checks" scope ends the run after P1 and its fixture's cleanup, with an
  `init vX.Y.Z --stop-after-p1` flag and a `brick_test.py stop-after-p1 "$test_run"` command mirroring the P0 ones; its
  `requested_verdict` follows P0 and P1 together. "Did P1 checks succeed?" is answered from the P1 verdict.
- **Permission preflight.** BRICK-TEST.md's up-front permission list names the live feedback send (outward-facing: it
  creates an issue in the maintainer's private feedback inbox) for any scope that includes P1, and not for a
  stop-after-P0 scope.
- **Documents.** BRICK-TEST.md (intro, preflight, product checks, verdicts, the stop scopes, and its "stop on a failure
  ... before continuing" rule, which gains the P1 exception), RECIPE.md (a P1 procedure section after the P0 one, the
  result template, the pass definitions), and root `AGENTS.md`'s "Release brick tests" section (which restates P0's
  rules) all describe P1. The coordinator's "Validate changes to this coordinator" section stays accurate.
  `releasing/AGENTS.md` step 8 mentions P1 only if it needs to stay true.
- **Tests.** `releasing/mac-vm-test/test_brick_test.py` covers the new priority: P1 fenced on a valid P0, P1 recorded on
  P0's last fixture before its cleanup (a P1 `fail` or `could-not-run` fenced on a valid P0 as well as a `pass`),
  `p1.verdict` reported apart from P0's, a P1 failure leaving `p0.verdict` a pass, stop-after-P0 refusing P1 records and
  still cleaning up P0's last fixture, stop-after-P1 ending after it, and a failed P1 neither blocking that pair's
  cleanup nor fencing bonus passes.
- The TODO.md entry ("Release-gate coverage for sending feedback.") is removed in the last PR.
- A linear stack of draft PRs, each passing its review gate. No PR is marked ready, and nothing is merged.

Out of scope: any change to the app, the helm, the feedback endpoint or the website handler (no test flag, label or
marker field: the title prefix is enough); keeping these issues out of the inbox repository (they land there, and the
maintainer filters notifications); running the brick test; the CI website job and the disabled browser job.

## Requirement sources

**The user's request (2026-10-14):** plan this Near term TODO entry. Its text:

> **Release-gate coverage for sending feedback.** A Farhelm release has to be able to send feedback to the endpoint that
> is live at farhelm.io, but nothing in the release gate checks that. The endpoint handler's tests run only in the
> on-demand CI website job, the browser test for the feedback dialog is in the disabled browser job, and nothing checks
> that what a release's helm sends is still accepted by the deployed endpoint, which ships separately and can change
> without an app release. Decide what the release gate should cover, including whether it sends to the live endpoint
> (and how such sends stay out of the maintainer's inbox) or tests against a local copy of the handler.

The maintainer's words with it: "for the feedback release gate lets bake it into the brick test immediately after the P0
category as a P1 category. it will simply do a real feedback submission with a pre-defined title etc so that i can
filter those out from notifications i have set up."

**The user's decisions (2026-10-14):**

- The first line, and so the title marker, is `[brick-test] vX.Y.Z` (issue title `Feedback: [brick-test] vX.Y.Z`).
- P1 runs on P0's last guest (`p0-updater-upgrade`), before that fixture's cleanup, rather than in its own fresh fixture
  or as part of a bonus pass.
- "Stop after P0" still means P0 only; a new "stop after P1" ends after P1; a P1 failure gets its own verdict, is
  reported right away, and does not block bonus coverage.
- Whether a P1 failure blocks promotion is the maintainer's call each time; the procedure writes no rule for it.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md "Feedback" (sending is a UI action only; no CLI or agent path may send
feedback; the displayed fields are the whole submission; a failed send keeps the dialog open); SPEC_impl.md "Feedback
forwarding" (the endpoint URL has no user-facing override; the title is `Feedback: ` plus the message's first non-blank
line, trimmed and shortened); `website/feedback/handler.js` (`TITLE_CHARS`: the shortening is to 60 code points, with
whitespace collapsed, so `[brick-test] vX.Y.Z` fits whole); root `AGENTS.md` "Release brick tests" (P0 first in its
five-step order, guest-only product mutations, permissions asked together up front, the guest text-entry workaround);
BRICK-TEST.md's own rules (P0 is never expanded with feature checks; observations are never overwritten);
`plans/AGENTS.md`, Executing one plan.

**Planner proposals:** the message body sentence, the pass oracle (the dialog closing itself, since the helm reports
success only after the endpoint answered that it created the issue, and a failure keeps the dialog open), checking the
displayed version and surface, the coordinator shape below, and the PR shape.

## Outline

The endpoint builds the issue title from the message's first non-blank line, so no app or endpoint change is needed. The
work is procedure and coordinator only:

- `releasing/mac-vm-test/brick_test.py`: `plan()` gives passes a priority (`P0` or `bonus`) and a `p0_sequence`;
  `checks()` maps P0 fixtures to their checkpoints; `create(p0_only)`, the ordering fence, `add_check` (which refuses
  P0), `limit_to_p0`, `require_priority`, `record()`, `cleanup()` and `report()` encode P0's rules. Find the code by
  name; line numbers drift.

  Today every gate judges a pass entry as a whole, so simply appending a P1 check to `p0-updater-upgrade`'s required
  checks would break the user's decisions in four places: (1) `p0.verdict` and `verdict(entry)` would count P1, so a P1
  failure fails P0 and a stop-after-P0 run (P1 never recorded) makes P0 "could not run"; (2) `cleanup()` requires the
  whole entry to pass, so that pair could not be cleaned, and `prepare()` refuses new fixtures while a pair is not
  cleaned, which blocks every bonus pass; (3) the run-wide stop-on-failure fences in `record()` and `require_priority()`
  refuse everything after any failure, P1's included; (4) `require_priority()`'s bonus fence checks P0 entries' whole
  verdicts.

  The smallest design that respects the decisions: keep P1 as one more check, last, in `p0-updater-upgrade`'s required
  checks, marked at check level with its own priority (for example
  `{'gui': True, 'command': False, 'continuity': False, 'priority': 'P1'}`, check name such as `p1-feedback-send`), and
  add one helper that restricts an entry to the checks of one priority (`report()` already does this ad hoc for
  `p0.checks`). Use it for the P0 verdict and the bonus fence, for `cleanup()`'s precondition (P0's checks pass, and P1
  is recorded either way or out of scope), for the stop-on-failure fences (a P1 failure does not trip them), and for
  `p1.verdict`. `record()`'s existing P0 prefix fence already fences a passing P1 on a valid P0 because P1 is last; that
  fence runs only for `status == 'pass'`, so a P1 `fail` or `could-not-run` needs the same fence. Refuse P1 records in
  stop-after-P0 scope. Nothing other than P1 may attach to P0's fixtures: `add_check` keeps refusing P0. A separate pass
  entry for P1 sharing the P0 pair is more machinery (pass aliasing across manifests, `pair()` and `record()`'s per-pass
  phase) and is not wanted.
- `releasing/mac-vm-test/test_brick_test.py`: the cases in the acceptance criteria.
- BRICK-TEST.md, RECIPE.md and root `AGENTS.md`: the text changes listed above. The P1 section in RECIPE.md gives the
  exact message, the GUI steps, the evidence and the pass rule.

Expected shape: one PR for the coordinator and its tests, then one for the documents and the TODO removal; one PR in
total is fine if the review reads it more easily that way. Type `test:` (it changes release-test procedure, nothing
users run); no changelog fragment, since nothing user-facing changes.

### Validation

Follow root `AGENTS.md` "Finishing work" and BRICK-TEST.md "Validate changes to this coordinator", choosing the smallest
set that covers the change: the coordinator's own test suite as that section says, `dprint check` on the changed
Markdown. Nothing here touches Rust, JavaScript or the website, so no runtime tests of those.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-feedback-brick-test-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing one plan, step 7) before any
work.

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

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/feedback-brick-test/<nn>-<short-name>` (for example `plan/feedback-brick-test/01-p1-coordinator`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `test`. No changelog fragment (nothing user-facing).
- The last PR removes the TODO.md entry this plan covers.
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
- the acceptance criteria: this file's goal, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a change to the app, the helm or the
  feedback endpoint, a separate P1 fixture, or a new evidence mechanism in the coordinator (examples, not a blacklist);
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
alternatives considered. In particular: the message sentence, the check's name, how the coordinator encodes P1 and its
priority helper, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

Agreed fallbacks: wording above that turns out inaccurate is corrected to the truth; if P0's last fixture cannot host P1
as described, pick the nearest shape that still runs P1 on that guest's candidate app without letting a P1 result change
P0's verdict, cleanup or fences, and log it.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: changing the app or the endpoint, adding a
label or test flag to submissions, running P1 in a different fixture, letting P1 failures block bonus passes, writing a
promotion rule for P1.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.
