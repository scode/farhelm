# A finished Restart and Replace with leave no stale state behind

Written against main at 1ba949ff on 2026-11-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Fix two leftover session-action state bugs from nonblocking-session-actions.** Found by its landing review
> (2026-11-10). A Restart that succeeded while the user was on another session is remembered until that session is next
> opened; if the host reboots again before then, opening it shows a terminal for a dead pane instead of the interrupted
> card with its Restart offer (`crates/farhelm-ui/src/session_view.rs`). Clear the remembered outcome when the listing
> shows the session running or its host gone. And Replace with's "Replacing…" progress has no cleanup guard
> (`create_form.rs`), so an abandoned request would leave it shown for the life of the page; plain Replace already uses
> a guard for this.

### What planning found (read from the code at 1ba949ff, not run)

**The remembered Restart.** The outcome is the `restart` field of `Lifecycle` (`crates/farhelm-ui/src/session_ops.rs`
around 37-41), kept in the page-owned `SessionOps.lifecycle` map (around 57), so it survives switching sessions. The
sidebar's code records it through `operations.boundary(...)` for plain Restart (`list/view.rs` around 3104-3143) and
Restart with (`restart_after`, around 3352-3360); `boundary` (`session_ops.rs` around 108-116) does not check whether
the session is open. Today it is cleared only by the open view's detail commit (`session_view.rs` around 1029-1043,
`fresh_confirms_run`): a fresh read showing the session stale or in any status other than Interrupted, with no request
pending. The whole entry is dropped when the session disappears from a complete listing (`list/view.rs` around
1992-1995). A view opened later seeds `relaunched` and `restart_terminal_kind` from the outcome (`session_view.rs`
around 621-647), and `terminal_absence` (around 3691-3699) treats Interrupted plus `relaunched` as "the listing has not
caught up" and mounts the terminal. After a second reboot every read says Interrupted, so the view attaches to a
destroyed pane.

In the sidebar's listing (`commit_listing`, `list/view.rs` around 1731), "running" is a status other than
`SessionStatus::Interrupted` and "host gone" is `session.stale`; a host removed from the registry removes the row, which
the existing cleanup already handles. The helm never lets the placeholder `Unknown` status overwrite a known one, so the
listing stays Interrupted until the restarted run is classified.

**Replace with's progress.** Set at `create_form.rs` around 6899-6904 in the page's `ReplaceProgress` map (`lib.rs`
around 1937-1939, whose comment already claims cancellation clears the entry); removed unconditionally after the reply
around 7024, and by `settle_prepared_source` (around 1068-1086) on preparation failures. If the launcher form unmounts
mid-request, its spawned task (around 6612) is dropped and the entry stays, keeping "Replacing…" on the row and header
(`session_view.rs` around 2734, 2848) and postponing deselection of a vanished source (`list/view.rs` around 2026).
Ordinary close controls refuse to close a submitting form, so no ordinary path reaches this today; the fix is defensive.
Plain Replace uses `ReplaceProgressGuard` (`replace.rs` around 106-125), created by the private
`ReplaceController::start_progress` (around 190-199): on drop it removes only the entry it inserted.

### Acceptance criteria

1. A remembered Restart outcome for a session is forgotten as soon as the sidebar's listing shows that session in a
   status other than Interrupted, or stale, while no request for it is pending. Opening such a session after a later
   host reboot shows the interrupted card with its Restart offer, not a terminal.
2. A Restart that succeeded while another session was open still shows the session's terminal when the user opens it
   before any listing has caught up (today's behavior, kept).
3. Replace with's "Replacing…" progress is removed when its request is abandoned, by the same guard plain Replace uses;
   a guard never removes an entry a newer request inserted.
4. Unit tests cover criteria 1 to 3. A browser test of criterion 1 is optional (see the planner proposals).
5. The report states the residual case (see Decisions).
6. The changelog is covered per root `AGENTS.md`, "Changelog fragments" (see Fallbacks).
7. The last PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-10 planning page:

- **Clearing rule:** the entry's own, as in criterion 1; it matches `fresh_confirms_run`.
- **Residual case accepted:** if the host reboots again before any listing saw the restarted session running or stale,
  the second interruption cannot be told apart from the moment right after the Restart, and the dead terminal still
  shows. The open session view has the same gap today. Closing it would need a reboot marker in the session listing (a
  protocol change); that is out of scope. State it in the report's "things you should know".
- **Reviewer:** a single fresh-context agent on Opus 5.5 at high effort (see Review gate). If it cannot be reached when
  a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled on hold**, at the maintainer's instruction on the planning page; it runs once the maintainer unholds it.

## Requirement sources

**The user's request and decisions:** the TODO entry and Decisions above.

**Binding repository constraints:** SPEC.md on a rebooted session still showing its interrupted card (search "A rebooted
session still shows its interrupted card"), results surviving session switches (search "Actions and their results
survive switching sessions"), and Replace progress disappearing on the reply; SPEC_impl.md on settlement clearing
progress and the session claim. None needs to change; if the fix contradicts any of them, the specs win. Root
`AGENTS.md`, "Tests and validation", "Changelog fragments", "Conventional Commits"; `.agents/test-authoring.md`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Apply the rule in the sidebar's always-mounted listing commit, sharing the predicate with `fresh_confirms_run` rather
  than writing it twice. Not inside the existing cleanup's block around `list/view.rs:1992-1995`: that block runs only
  for complete, unfiltered, untruncated listings (`authoritative && absence_is_evidence(listing)`), and clearing needs
  only a positive observation of the session, so inside it the bug would survive for a filtered or truncated list. Keep
  it behind the superseded-read drop and `authoritative`. A pure helper that retires outcomes against a listing is
  testable without the UI framework.
- No fencing of listing reads against Restarts. Confirm, rather than assume, that the generation drop at the top of
  `commit_listing` keeps a listing read from before the Restart from committing after the reply and clearing the outcome
  early (criterion 2). If you find a real race, log it and fix it narrowly.
- Make `ReplaceProgressGuard` `pub(crate)` with a constructor taking the `ReplaceProgress`, the session id and the
  `PendingLaunch`, and use it in Replace with where the entry is inserted; drop it where the reply now removes the
  entry, before `on_settled`, with no await in between (the constraint plain Replace documents around
  `replace.rs:511-514`). Leave `settle_prepared_source` alone unless the guard makes its removal wrong. The existing
  `replace_progress_is_shared_and_cleared_by_request_ownership` test (`replace.rs` around 994) covers drop-clears; the
  new unit test needed is that an older guard leaves a newer request's entry alone.
- Browser test, a stretch rather than a requirement: `injectInterruptedSession` (`e2e/tests/terminal-restart.spec.ts`
  around 59-149) intercepts the listing and the restart route. Making its listing state mutable (interrupted, running,
  interrupted again) and holding the restart reply until another session is open can stage criterion 1; the injected
  session's detail read may also need routing. If staging it needs more than a modest fixture change, rely on the unit
  tests and say so in the report. No browser test for the abandoned Replace with: the form cannot be closed while it
  submits.

## Outline

1. `plan/session-action-leftovers/01-forget-restart-outcome`: criteria 1, 2 and their tests. `fix:`.
2. `plan/session-action-leftovers/02-replace-with-progress-guard`: criterion 3 and its test, and the TODO entry removal.
   `fix:`.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-ui`, the UI crate's unit tests through `python3 scripts/record-test-run.py`, any changed browser spec on
Chromium and WebKit, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` when tests change, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

If another plan has moved the code this file names, follow the current code and correct this file's names and line
numbers to the truth, logged as a DECISION. If main already fixes either bug, the plan is done except for whatever
criteria remain; finish those.

Changelog: both bugs come from nonblocking-session-actions. If `releasing/changelog.d/nonblocking-session-actions.md`
still exists when you run (that work has not been released yet), these are follow-ups to unreleased work: edit that
fragment rather than adding one, if anything user-facing changes in what it says. If it has been released, add a `fixed`
fragment for the Restart fix; the Replace with guard has no user-facing effect on any ordinary path.

Examples of departures that need the scope reassessment: a reboot marker or any protocol change, fencing listing reads,
or reworking how the session view decides between terminal and interrupted card.

Decisions to log: where the clearing runs and how the predicate is shared, how the guard is exposed, whether the browser
test was staged, the changelog handling, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-session-action-leftovers-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md`, "Executing one plan" (step 7)
before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality (independent investigations in parallel, say). Concurrent agents that might otherwise conflict
can work in separate jj workspaces whose results you merge back; you still own the stack and every PR. Launch a
sub-agent natively, with model and effort set explicitly where the mechanism allows; when the model you need is not
available as a native sub-agent type, `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) run as that file says: native, read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start a resource watchdog as a
background process (not an agent) and keep it running for the whole run. Every 60 seconds it samples free space on the
filesystems holding the checkout, any jj workspaces you create, the agent scratch directory and `/tmp`, plus available
memory and swap (`df`, and `free` or `/proc/meminfo` on Linux; `vm_stat` and `sysctl` on macOS). It writes each sample
to a private heartbeat/status file in the scratch directory, and it emits a notification when any of these happens:

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

On every path, check heartbeat freshness before each new build, test run, sub-agent or review launch. A dead monitor, or
a heartbeat stale for two sample intervals, pauses new launches until monitoring is restored; restart a dead watchdog.

An alert is an instruction to act:

1. stop launching sub-agents and work;
2. remove jj workspaces, build output and scratch you own that are no longer needed;
3. wait for or stop the job most likely responsible;
4. resume only when the watchdog reports headroom.

Record the watchdog's handle, watched paths, status path and delivery mechanism in the log. Include its state in every
handoff note so a resumed session reconciles or restarts it, and stop it when the plan closes. Do not lengthen the
sampling interval to save turns.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/session-action-leftovers/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- The plan's last code PR removes this plan's TODO.md entry.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high effort.
Launch it as a native sub-agent with that model and effort when your harness serves it, and otherwise through
`$scode-harness-shellout`. If it cannot be reached when a review is due and is still unreachable after a reasonable
retry, block the plan per `plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is
consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. A sub-agent gets
a bounded unit within the outline; an unapproved new mechanism is outside its task, and one discovered mid-unit is left
unimplemented and reported back, with checks and incomplete work stated honestly. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists in the order the outline gives, together meeting the
acceptance criteria above (as refined by Decisions already made), each having passed its review gate, with the last one
removing this plan's TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be
satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its
report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
