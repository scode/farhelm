# Launches that wake a host no longer leave the page stuck

Written against main at 1ba949ff on 2026-11-10. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `sandbox-suspend-overlap.md`, whose second PR changes the same wake code and the error the wake
reports. Build on what that plan landed; the line numbers below are from before it.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Stop two launch paths from leaving the page stuck after waking a host.** Found by the landing review of
> nonblocking-session-actions (2026-11-10) and landed unfixed as narrow cases. First: a quick launch whose template
> checks out a GitHub repository on a suspended sandbox host, if its retry after the wake is refused without an error
> message, leaves an invisible launcher over the page that blocks every click and key until Escape is pressed. The
> reveal check runs while the launch token is still held and never runs again once it is released
> (`crates/farhelm-ui/src/list/create_form.rs`, the quick-launch reveal effect and the continued-submit branch). Second:
> a launch that woke a host waits with no time limit for the page to see that host connected; if it never does, New,
> quick launches and host changes stay refused and the hidden launcher has no Cancel. Before that plan the launcher was
> visible and could be cancelled there. Give that wait a limit that returns the launcher with an error.

### What planning found (read from the code at 1ba949ff, not run)

**The invisible launcher.** The quick-launch reveal effect (`create_form.rs` around 5434-5443) un-hides the launcher
when there is an error, a template error, an open YOLO question, or an attempted launch while `!ops.busy_now()`.
`busy_now()` reads the token with `peek` (`crates/farhelm-ui/src/ops.rs` around 73-75), so the effect never re-runs when
the token is released. The continued submit after a wake (around 5633-5676) sets the attempt to Refused and clears the
error while it still holds the token, so the effect sees "busy" and does not reveal. Several checks then return without
an error and rely on the launcher's rendered prerequisite text (harness missing around 5776, remembered folder mismatch
6016, folder default pending 6024, host not available for launch 6044, no preview authority 6064). The continued
branch's end (around 7227-7241) spawns `return_prepared_composer` (around 2004-2016), whose admission makes the
still-hidden form modal and isolating before the guard drops. Result: a hidden modal dialog; Escape works because the
form's escape target is its own Cancel. Error refusals already reveal before admission (for example the resume failure
around 6146-6162).

**The unbounded wait.** The wake itself is bounded: `api::resume_host_connected` (`api.rs` around 5666-5701) has the
resume POST's 150 s budget, then 60 s of polling for Connected, failing with "The host resumed but did not connect
within 60 seconds; try again when it connects". On success, the fresh-checkout path (`create_form.rs` around 6052-6162)
parks the stamp and guard in `guarded_submit_pending` (around 6129-6136) and leaves `resuming_host` set. The effect
around 7326-7395 then waits, with no limit, while the form still shows the old connection (`waiting_for_connection`,
around 1120-1124) or the selected host still shows a connection pending (around 7356-7358, `resume_display_observation`
around 259-268): both come from the page's own host list. The parked guard holds the token, so the list refuses New,
quick launches and host changes; the form stays hidden and the progress pill has no Cancel (SPEC.md says the pill takes
neither focus nor pointer input). `discard_guarded_preparation(Some(reason))` (around 7292-7322) already settles the
source, sets the error and returns the launcher. The existing-folder wake path (around 6641-6685) binds straight from
the helper's fresh host snapshot and has no page-side wait.

### Acceptance criteria

1. A quick launch with a GitHub checkout on a suspended sandbox host, whose continued submit after a successful wake is
   refused without an error message (any of the silent refusals above), returns its launcher visibly with its draft and
   the prerequisite text explaining the refusal. No hidden launcher blocks page input. A browser test shows this on
   Chromium and WebKit.
2. After a successful wake for a fresh-checkout launch, the page's wait for its own host list to show the woken host's
   new connection lasts at most 60 seconds. When it runs out, the parked submission is discarded and the launcher
   returns with its draft and exactly this error: "The host resumed, but this window did not see it connect within 60
   seconds. The session was not launched; try again." New, quick launches and host changes are admitted again. A
   connection that shows up after the limit creates nothing. A connection seen within the limit continues as today. A
   browser test shows the timeout on Chromium and WebKit, using the page clock rather than real waiting.
3. The limit covers only that page-side wait. The wake's own budgets and checkout preparation keep theirs unchanged.
4. SPEC.md ("Creation", the paragraph on waking a suspended sandbox before creating) and SPEC_impl.md ("GUI: Dioxus",
   the hidden-launch paragraphs) state the limit. That a refused continuation returns the launcher already follows from
   SPEC.md ("A failure, unknown outcome or follow-up question restores the same launcher with its draft", and "Keyboard
   focus" on a hidden launcher showing an outcome); add text for it only where the specs turn out to be silent.
5. The changelog is covered per root `AGENTS.md`, "Changelog fragments" (see Decisions).
6. The last PR removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-10 planning page:

- **Order:** after `sandbox-suspend-overlap.md`. Build on its wake changes and its wording for wake failures. Rejected:
  running independently, and folding the time limit into that plan.
- **The limit:** 60 seconds, covering only the wait for this window to see the woken host's connection. Rejected: 30
  seconds, and a limit over the whole hidden continuation including checkout preparation.
- **Error wording:** "The host resumed, but this window did not see it connect within 60 seconds. The session was not
  launched; try again." If `sandbox-suspend-overlap` changed the wake's own wording, keep this message consistent with
  it in style without changing its content.
- **Reviewer:** a single fresh-context agent on Opus 5.5 at high effort (see Review gate). If it cannot be reached when
  a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled on hold**, at the maintainer's instruction on the planning page; it runs once the maintainer unholds it.

## Requirement sources

**The user's request and decisions:** the TODO entry and Decisions above.

**Binding repository constraints:** SPEC.md "Topology" (launching on a suspended host; a failed preparation says the
session was not launched), "Creation" (a failure restores the same launcher with its draft; a successful wake continues
without revealing the launcher), and "Keyboard focus", "Opening and closing" (hidden quick-launch preparation; a pending
launch's hidden launcher releases isolation until it needs to show an outcome); SPEC_impl.md "GUI: Dioxus"; root
`AGENTS.md`, "Tests and validation", "Changelog fragments", "Conventional Commits"; `.agents/test-authoring.md`. No real
Tensorlake calls: browser tests use the existing stand-in fixture.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- First fix: in the continued branch, reveal the launcher (`quick_launch_hidden.set(false)`) before spawning the return,
  as error refusals already do. Rejected for now: making the reveal effect subscribe to the token (`ops.busy()`), which
  touches every token toggle.
- Second fix: when the fresh-checkout path parks its submission (right after `guarded_submit_pending.set(...)`, around
  6131), spawn one 60-second wait with the existing `crate::reader::sleep_ms` (`reader.rs` around 603; it wraps gloo's
  `TimeoutFuture` on wasm, which `page.clock` controls). When it ends, act only if the parked submission is still this
  one and still in one of the two connection waits (`waiting_for_connection` on live preview authority, or a resume
  still showing its display observation); a submission that saw its connection and now waits on checkout preparation
  must not be cut off (criterion 3). Then call `discard_guarded_preparation` with the agreed error. The parked stamp has
  no unique id; a one-field generation counter avoids a quick retry in the same opening being timed out by an earlier
  attempt's timer. Do not add a general timeout facility.
- Browser tests live beside the existing wake-to-create cases in `e2e/tests/tensorlake-lifecycle.spec.ts` ("a suspended
  fresh checkout uses one press", its `quick` mode, and the `fleet()` fixture). The fixture's `holdNextRead()` keeps the
  page from seeing the connection; combine it with `page.clock`, as `e2e/tests/launch-progress.spec.ts` does. For a
  silent refusal, publish a settled non-connected host phase with the real connection while the preview is held, then
  release it (plausible from reading the fixture, not tried). If staging either case needs more than a modest fixture
  change, cover the fix with the tests that can stage it (unit tests included) and say so in the report rather than
  blocking.
- After `sandbox-suspend-overlap`, a host suspended again during this page-side wait still ends in the 60-second error,
  which says the window did not see it connect rather than that it was suspended. Do not add fast failure here; mention
  it in the report's "things you should know".

## Outline

1. `plan/wake-launch-stuck/01-reveal-refused-retry`: criterion 1, its browser test, and its part of criterion 4. `fix:`.
2. `plan/wake-launch-stuck/02-connection-wait-limit`: criteria 2 and 3, the timeout browser test, the rest of criterion
   4, and the TODO entry removal. `fix:`.

If both turn out to be a few lines each, one PR is fine; log the DECISION.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
`farhelm-ui`, the affected UI unit tests through `python3 scripts/record-test-run.py`, the changed browser spec files on
Chromium and WebKit, `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md`, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

If `sandbox-suspend-overlap` or another plan has moved the code this file names, follow the current code and correct
this file's names and line numbers to the truth, logged as a DECISION. If main already fixes either stuck state, the
plan is done except for whatever criteria remain; finish those.

Changelog: the two stuck states were introduced by nonblocking-session-actions. If
`releasing/changelog.d/nonblocking-session-actions.md` still exists when you run (that work has not been released yet),
these are follow-ups to unreleased work: edit that fragment rather than adding one, if anything user-facing changes in
what it says. If it has been released, add a `fixed` fragment.

Examples of departures that need the scope reassessment: a Cancel on the progress pill (SPEC.md says it takes no pointer
input), a time limit on other launch paths, or reworking how the reveal effect tracks the token.

Decisions to log: how the first fix reveals, how a parked submission is identified for the timer, how each test stages
its case, the changelog handling, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-wake-launch-stuck-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/wake-launch-stuck/<nn>-<short-name>`, as
  the outline names them.
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
