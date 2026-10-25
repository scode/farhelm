# UI: report the settings saves that still fail silently

Written against main at aa245430 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

SPEC.md (around 3434-3437, set 2026-10-22) says a failed GUI save is reported, then discarded, and the GUI must not keep
it as the current state. SPEC.md (around 2845-2861) lists the only saves that may fail silently. Six saves on neither
side of that rule still fail silently. The TODO.md entry (bucket "Near term"), verbatim:

> **Report every failed settings save.** SPEC.md asks for every failed save to be reported, but several still fail
> silently: the welcome card seen, trust-hint dismissals, the terminal link-click choice, frame colors, naming sessions
> on quick launch, and toggling the ⚡ mark. The follow-up the maintainer chose to leave out of
> `sign-in-preservation-sweep` (review of 2026-10-23).

What exists, from a read of main at aa245430 (line numbers approximate, all under `crates/farhelm-ui/src/`):

- `api::store_preference` (`api.rs` around 1214) never reports a failure; it only logs (around 1263).
- `api::preference_write_completion` (around 1303) is private. It returns true only when the helm acknowledged the
  write, and also returns false when a newer write of the same field superseded it (around 1027), so a caller needs a
  generation guard to tell a superseded write from a failed one. It carries no error text.
- Two public wrappers already report: `restore_session_confirmations` (around 1295), whose Settings switch shows an
  alert under itself with a generation counter (`settings.rs` around 249-281); and `write_template_binding` (around
  1278), with which template number keys put back the helm's value and show `.templates-error` (`list/templates.rs`
  around 1669-1686).
- There is no app-wide notice for failed saves.

The six silent sites:

| Site                                        | Where                                                         | What reporting takes                                                                         |
| ------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| ⚡ mark toggle                              | `list/templates.rs` around 1688                               | the number-key arm's pattern, with `write_template_binding`                                  |
| Frame colors                                | `settings.rs` around 304-319                                  | the session-confirmation switch's pattern                                                    |
| Naming sessions on quick launch             | `settings.rs` around 283-297                                  | the same                                                                                     |
| Terminal link-click choice                  | `settings.rs` around 823-853                                  | the same; the three radios are separate components, so the error state moves to their parent |
| Trust hints, Settings switches              | `claude_trust.rs` around 120, `workspace_trust.rs` around 217 | an `error` signal and alert line already exist                                               |
| Trust hints, "Don't show again" on the band | `claude_trust.rs` around 185, `workspace_trust.rs` around 290 | see Decisions                                                                                |
| Welcome card seen                           | `app_bar.rs` around 616                                       | see Decisions                                                                                |

SPEC.md's Welcome card section (around 2906-2907) already says "A failed write still closes it and may let it appear on
a later load", and SPEC_impl.md (around 1212-1213) says the ⚡ toggle's failure "is not yet reported".

History: `sign-in-preservation-sweep` (#2604, #2614, #2623) made failed saves be discarded rather than resent. On
2026-10-22 the maintainer chose to leave this reporting gap out of that plan, over adding these saves to the silent list
or reporting each one; this entry is that follow-up.

### Acceptance criteria

- Frame colors, naming sessions on quick launch, the terminal link-click choice and the two trust-hint Settings switches
  each report a failed save with an alert line under the control, and put the control back to the value it had before
  the click (the value the helm still holds). Only the alert and generation-guard half follows the session-confirmation
  switch; that switch never flips back, so the flip-back is new at each site. A write superseded by a newer click of the
  same field is neither reported nor reverted; only the latest write's outcome counts.
- If Settings is closed before a failure arrives, the revert still happens and only the alert is dropped, so the GUI
  never keeps the change it tried to save as current state (frame colors, naming and the link-click choice write their
  new value into the shared preferences immediately, so a revert tied to the Settings panel would leave it there). Say
  so in SPEC.md beside the session-confirmation sentence that "closing drops the error".
- Toggling the ⚡ mark reports a failed save the way template number keys do, and puts back the helm's value.
- A trust hint's "Don't show again" clicked on the band itself: on a failed save, the band comes back with the error
  while its session is still on screen. If the user has moved away by the time the failure arrives, the failure stays
  silent, and SPEC.md says so.
- The Welcome card's "seen" save joins SPEC.md's list of saves that may fail silently, consistent with its own section's
  sentence. Its code stays silent; `e2e/tests/welcome.spec.ts` (around 134), which asserts the silent close, keeps
  passing.
- Fixed sentences for the errors, in product terms (root `AGENTS.md`, "Talking to the user"), since a failed save
  carries no error text. Follow the wording of the existing session-confirmation and number-key errors.
- SPEC.md and SPEC_impl.md updated: the silent list (welcome seen; the band's "Don't show again" once its session has
  left the screen), the sentence in SPEC_impl.md that the ⚡ failure is not yet reported, and any other sentence that
  describes these saves as silent.
- Device-local saves (the sound switches, sidebar width, text size) are out of scope; they do not go to the helm.
- Browser tests on both engines, modelled on the session-confirmation failure tests
  (`e2e/tests/session-confirmations.spec.ts` around 60-150) and the number-key failure tests
  (`e2e/tests/templates.spec.ts` around 342-387): one table-driven test over the five Settings-panel controls (refuse
  that field's write, click, expect the alert and the old state), one case showing a superseded write is not reported,
  one ⚡ case, one band case (Claude or workspace, not both), and one case showing a revert survives closing Settings.
  Re-check `e2e/tests/quick-launch.spec.ts` (around 1037-1110) and `e2e/tests/restart-with.spec.ts` (around 1955) if the
  ⚡ toggle's timing changes.
- One changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`),
  added in the first PR and edited by later PRs of this stack as they add sites.
- The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **On a failed save the control flips back and the failure is reported**, following the 2026-10-22 rule literally, over
  reporting only and leaving the control as clicked.
- **Welcome card seen goes on the silent list**, over reporting it through a new notice shown after the card has closed.
  A failure only means the card shows again on a later load, which its section already allows.
- **Trust-hint "Don't show again" on the band: re-show the band with the error** while its session is still shown;
  silent once the user has moved away, and SPEC.md says so. Chosen over putting the band's save on the silent list.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

Stated to the maintainer on the planning page, without objection: fixed sentences; device-local saves out of scope;
about 100-150 lines plus browser tests, in 2-3 PRs grouped by surface.

Settled by the planner after the planning review, as following from the flip-back decision: a revert outlives the
Settings panel (see Acceptance criteria).

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's failed-save rule (around 3434-3437), its silent list (around 2845-2861)
and Welcome card section (around 2906-2907); SPEC_impl.md's preference-write paragraphs and the ⚡ toggle sentence; root
`AGENTS.md`, "Talking to the user", "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and
FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Expose the outcome through one generic `pub(crate)` acknowledgment wrapper around `preference_write_completion`
  (`write_template_binding` is already generic in all but its name and could become it), rather than one wrapper per
  field or making `store_preference` report.
- Reuse the existing generation-counter pattern for supersession. Note its blind spot: the trust-hint band and its
  Settings switch write the same field, and a per-component counter cannot see the other's click. If that case matters
  in practice, a completion that tells acknowledged, failed and superseded apart (the write queue already knows which)
  would replace the per-site counters; that touches shared queue code, so take it only if it is the smaller fix, and log
  the choice.
- The trust-hint switches and bands derive their state from a fetched resource and already re-read after every click
  (`claude_trust.rs` around 118-122 and 184-187, `workspace_trust.rs` around 217 and 290). There, reverting is setting
  the error and re-reading; no previous value needs capturing, and the band reappears by itself once the read shows it
  is not dismissed. A task owned by the session view (cancelled when it unmounts) gives "silent once the user has moved
  away" with no extra state.
- For the Settings-panel controls, the revert runs in a task that outlives the Settings panel (only the alert belongs to
  the panel).
- No app-wide notice. Every report sits beside the control that was clicked.

## Outline

A stack of three PRs:

1. `plan/report-failed-settings-saves/01-settings-panel`: the wrapper(s), and reporting for frame colors, naming on
   quick launch, the link-click choice and the two trust-hint Settings switches; their tests; the changelog fragment.
2. `plan/report-failed-settings-saves/02-lightning-mark`: the ⚡ toggle; its tests; the SPEC_impl.md sentence; the
   fragment edited.
3. `plan/report-failed-settings-saves/03-trust-band-welcome`: the band's "Don't show again" and its tests; the SPEC.md
   silent-list changes for the band and the welcome card; the fragment edited; the TODO.md removal.

Merge or split differently if the code makes another cut more reviewable, and log it.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-ui`, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares, and
through `python3 scripts/record-test-run.py` the new and changed browser tests on both engines, plus the existing
settings, trust-hint, quick-launch, template and welcome specs the changes touch.

## Fallbacks

Examples of departures that need the scope reassessment: an app-wide notice or toast system, making `store_preference`
itself report, retrying failed saves, reporting device-local saves, or reporting the welcome card.

Decisions to log: the wrapper shape, each error sentence, the PR cut, every test changed and why, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-report-failed-settings-saves-log.md` in the parent directory of the checkout you run in, derived as that
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
available as a native sub-agent type (the reviewer below, for instance), `$scode-harness-shellout` supplies the launch
mechanics.

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
  `plan/report-failed-settings-saves/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `review_feedback_queue/` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched through
`$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached when a review is
due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`, "Executing one
plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

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

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes this plan's TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md`,
"Executing one plan" (steps 11 and 12): deliver its report through the queue script, write a closing entry in its log,
and stop the watchdog. Never edit `plans/` yourself.
