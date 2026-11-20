# Remember a session's active terminal

Written against main at 0055c6beb on 2026-11-19. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says.

## The goal

The TODO.md entry (bucket "Near term", subsection "Trivial"), verbatim:

> **Remember the active terminal per session.** when you're in a session and you select or start a terminal and the
> terminal is active, it should be remembered as the active terminal fro that session instead of always switching back
> to the main agent terminal when you switch away and then back

### Where it stands (from planning, at main 0055c6beb)

Which of a session's terminals is shown lives only in the desktop app, as `TerminalSet::chosen`
(`crates/gui/farhelm-gui/src/terminal/set.rs`, `None` for the agent terminal). `SessionPane::sync` drops the whole
mounted view, the terminal set included, whenever the selected session changes, and builds a new one
(`crates/gui/farhelm-gui/src/session_pane.rs`, module docs and `sync`); `TerminalSet::new` starts with `chosen: None`.
The module NOTE in `set.rs` ("Selection is not kept across a change of session...") states the current behavior. It was
ported from the old Dioxus app, which also reset on every switch (#3663); no SPEC.md text, lore entry, triage outcome or
commit message chose it as a product rule. The helm records nothing about which terminal a client shows, and tab ids are
minted by the supervisor and never reused (`crates/farhelm-proto/src/lib.rs`, `TabInfo`), so a stale id can never alias
a new tab.

The one entry point for a choice is `TerminalSet::choose` (strip buttons, and `choose_in` once a tab opened with New
terminal is listed and the user has not moved on). The fallbacks to the agent terminal (the chosen tab stops being
listed in `follow_tabs`, or a close of it succeeds) do not go through `choose`.

There is no general per-session view-memory store. `LocalState` (`local_state.rs`) must not grow into one (its docs and
SPEC_impl.md say so). The nearest patterns are two small session-keyed survivors: `Carried` in `terminal/attachments.rs`
(an app global that survives resets, listed in SPEC_impl.md as the "second, deliberate survivor") and `TrustBand.hidden`
in `session_pane/trust_band.rs`.

### Acceptance criteria

1. Choosing a terminal tab in a session (a strip click, or a tab opened with New terminal that the user waited for),
   switching to another session, and coming back shows that tab, and keystrokes reach it.
2. Choosing Agent in the strip clears the memory: coming back shows the agent terminal.
3. A remembered tab that is gone when the session is opened again (closed, reaped, the session stopped) falls back to
   the agent terminal silently. The entry is not dropped then: tab ids are never reused, so a stale entry can never
   match a new tab, and dropping it at restore would let a set built while the tab list is briefly short wipe the memory
   for good. Only choosing Agent removes an entry.
4. Every way of opening a session restores the same way: a row click, the quick switcher, an automatic open by the
   selection rules. Clicking the session that is already open does not reset it (it focuses the shown terminal, as
   SPEC.md says today). Focus follows whichever terminal is shown, through the existing focus path.
5. The memory restores even when the agent is waiting on the user.
6. The memory lasts for the app's lifetime: it survives a reconnect to the helm (a connection reset that rebuilds the
   view), and is not kept across quitting and reopening the app.
7. A remembered tab that is past the 32-tab attach cap, or otherwise has no attached terminal at restore time, is not
   restored (the agent terminal is shown), so a set never starts on a terminal that nothing draws or measures.
8. SPEC.md "Session view" gains a sentence saying the view remembers which terminal the user last chose in each session
   for as long as the app runs, falling back to the agent terminal when that tab is gone; it stays consistent with
   "Choosing a session ... focuses its terminal" and the shell-tab-exit focus rule. SPEC.md "Losing the connection to
   the helm", which today lists the selected session as the only thing that survives a reset ("Everything else starts
   over ... Nothing is replayed, restored or merged back"), adds the remembered terminal to that list, as the reset
   decision requires. SPEC_impl.md names the new survivor beside `Carried`, and the "only survivor" wording in the
   module docs of `crates/gui/farhelm-gui/src/session.rs` is updated to match.

## Decisions already made

Settled with the maintainer on the 2026-11-19 planning page:

- **Restore when the agent needs attention:** yes, always restore. Nothing in v1 opens a session because it needs
  attention (the bell and approval cards do not change the selection).
- **Across app restarts:** no, only while the app runs. Storing it on the helm was offered and declined.
- **Across a dropped connection to the helm:** yes; snapping back to Agent after a reconnect would look like the same
  bug.
- **Release note:** "No release note needed." (no highlight; the fragment still describes the change for the full list).
- **Review gate:** a single fresh-context gpt-6.2-sol agent at high effort with the general review charter.
- **No-workhorse mode**, the plans default, stated on the planning page.

Planner decisions shown on the page without objection: all opening paths restore alike; a gone tab falls back silently;
re-clicking the open session does not reset it.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the picks above.

**Binding repository constraints:** `local_state.rs` and SPEC_impl.md's rule that `LocalState` holds only the selected
session; SPEC.md "Session view" and "Keyboard focus" (choosing a session focuses its terminal; a shell tab exiting while
typed in hands focus to the agent tab); root `AGENTS.md` "Conventional Commits", "Changelog fragments", "Tests and
validation"; `.agents/validation.md`; `.agents/test-authoring.md`.

**Planner proposals** (verified against the code and reviewed by an independent planning reviewer):

- **One app global in `terminal/set.rs`**, for example `ShownTerminals(HashMap<session key, tab id>)`, written only in
  `TerminalSet::choose` (insert for a tab, remove for Agent), read once in `TerminalSet::new` after its first
  `follow_tabs`. Recording only explicit choices, rather than mirroring `chosen`, keeps a transiently empty tab list or
  a reap fallback from wiping the memory; the restore-time check against the listed, attached tabs makes stale entries
  harmless. An app global, like `Carried`, survives resets by construction, which is what the reset decision asks for.
- **No pruning.** An entry goes only when the user chooses Agent. Entries for tabs that are gone, and for sessions
  deleted while away, stay until the app quits (two short strings each) and are never restored, because tab ids are
  never reused. Do not add pruning through `session::listing_proves_gone` or otherwise; nobody asked for it.
- **Leave the remount alone.** Keeping terminal sets attached across selections would also keep scroll position, but it
  contradicts the pane's documented design and multiplies attached streams. Not in scope.
- **Tests** in `crates/gui/farhelm-gui/src/terminal/set/tests.rs`, modeled on
  `choosing_another_session_ends_every_terminal_of_the_open_one`: a restored tab gets the keystrokes; Agent clears the
  memory; a tab gone while away falls back to Agent and is focused; a New terminal tab is remembered; a tab past the cap
  is not restored. In `session_pane/tests.rs` "Resets": a reset that keeps the session listed with its tab reattaches
  and shows the tab. Update the parity map `docs/gpui/parity/terminal.md` with the new test names.

## Outline

One PR: `plan/remember-active-terminal/01-remember-terminal` (`feat:`): the global and its two touch points, the tests,
the replaced NOTE in `set.rs` and the adjusted comments in `session_pane.rs` that say the pane always starts on the
agent terminal, the `session.rs` module docs, the SPEC.md and SPEC_impl.md sentences, an optional line on
`website/src/content/docs/docs/using/work-in-a-session.mdx` (read `website/AGENTS.md` first; an incidental docs edit
starts no preview server), the changelog fragment (`kind: changed`), and the TODO.md entry removed.

## Fallbacks

- If restoring a tab as the first shown terminal makes the agent terminal attach later in a way an existing test pins
  (for example a test that expects the agent stream first), keep the behavior the acceptance criteria ask for, adjust
  the test with a docstring saying why, and log it.
- If the pane needs a second touch point (for example to make the restored tab home before focus), that is within scope;
  log it.

Decisions to log: where the global lives and its name, and every review finding declined with its reason.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-remember-active-terminal-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-remember-active-terminal-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Apply `.agents/test-authoring.md` to every test you add or change, and run
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when Rust tests change.

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

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
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/remember-active-terminal/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing words what that PR
  built, with the kind that fits it (a user-visible change is `changed` or `added`, not `none`, even when the release
  note answer is "No release note needed."). Where what got built departs from a release note entry, the fragment's
  context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last PR of the stack removes this plan's TODO.md entry.

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
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

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
working log with the true value; never edit this plan file.

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
meeting the acceptance criteria above, each having passed its review gate, and the last PR removing this plan's TODO.md
entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the
plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
