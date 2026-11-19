# Clear refusals for an unusable folder or size

Written against main at d480779c on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Make terminal hosts refuse an unusable folder or size clearly.** Known gaps in SPEC_impl.md since
> tmux-end-old-terminals (#3358 to #3370): creating a session or opening a tab in a folder that cannot be entered fails
> with an error that names `systemd-run` rather than the folder, and attaching at size 0x0 is mishandled. Terminal hosts
> should refuse both up front with a clear message, the SPEC_impl.md known-gaps entries go, and the three tests that
> were lost with tmux come back on terminal hosts. The same plan also lost, without porting, the check that memory stays
> flat while a slow viewer stalls and two file-upload tests beside a busy or deleted session; port those with this.
> Recorded by the maintainer on 2026-11-18 from the plan's review.

In product terms, explained to the maintainer while planning:

- Launching a session or opening a tab in a folder that exists but that the user is not allowed to enter is refused with
  an error naming an internal tool (`starting /usr/bin/systemd-run: Permission denied`) rather than the folder. The fix
  is to say that the folder cannot be entered, and name it.
- A viewer asking for a zero-size terminal is quietly ignored today; the desktop app never asks for one. The fix is a
  clear refusal.
- Six tests deleted with tmux and never ported come back on terminal hosts.

The maintainer's instruction when confirming it, verbatim: "assuming it's not very complicated if it is tell me". The
planner judged it not very complicated; the one part it could not size is the memory test (criterion 4).

### Acceptance criteria

1. Creating a session, restarting one, and opening a tab check, before starting anything, that the folder can be entered
   (search permission) by the user the host runs as, not only that it exists and is a directory. A folder that cannot be
   entered is refused the way a missing folder is refused today: a request error before any session row exists for a
   create (so no session is left, not even one in Error), a refused open for a tab. The refusal uses the existing
   "working directory is not usable: <folder> (...)" form that `ensure_cwd_usable` already uses for other unusable
   folders, so the launcher's existing recognition of those words marks the folder field (SPEC.md) with no app change.
   No session or tab is left half made, and the agent never starts in another folder.
2. The check is in the supervisor, in `ensure_cwd_usable` beside the existing existence check, so the terminal host
   program, which outlives every release (`crates/terminal-host/SPEC.md`, "Change policy"), does not change for it. The
   launcher's advisory folder check (`ensure_browse_cwd_usable`, a near-copy) is left as it is, and the two are not
   merged. A permission change between the check and the start still shows today's message; SPEC_impl.md says so rather
   than adding machinery for it.
3. An attach that asks for a terminal of zero rows or zero columns is refused through the attach's existing error reply,
   with a clear message, instead of attaching with only a warning in the log. The size is checked before anything else
   the attach does, so a refused attach never displaces the viewer already attached. The terminal keeps its size and
   keeps running. A resize (which has no reply) keeps logging a warning, and a create's starting size keeps being
   clamped to at least 1; neither is part of this criterion, and SPEC_impl.md says so.
4. The six tests come back, on terminal hosts, each guarding what it guarded under tmux:
   - a create in a folder that cannot be entered is refused with an error naming the folder, no session is listed, and
     the agent never runs elsewhere (`create_records_error_when_the_session_directory_cannot_be_entered`, renamed to
     match: it no longer records a session in Error);
   - a tab opened in such a folder is refused with an error naming it, no half-made tab is left, and the session keeps
     running (`a_tab_refuses_a_directory_it_cannot_enter`);
   - a zero-size attach (`attach_with_degenerate_size_still_works`), rewritten for criterion 3's refusal: the request is
     refused, and the terminal still shows output at its size;
   - memory stays flat while a viewer stops reading a terminal that produces a lot of output
     (`memory_stays_flat_while_a_viewer_is_stalled`);
   - a file upload's replies are not stuck behind a busy terminal's output
     (`an_ack_arrives_ahead_of_a_backlog_of_terminal_output`);
   - a refused Delete leaves an upload already in progress usable for a retry
     (`a_refused_delete_preserves_an_in_flight_upload`).

   Their tmux versions are in `git show 647f0227b^` (PR #3360 deleted them; its commit message lists them as lost
   coverage), with the test helpers deleted alongside them: the `DeniedDirectory` fixture (restore it as it was), the
   memory test's measurement helper (`terminal_backpressure/rss.rs`), its fake-agent scripts (`flood-memory`,
   `flood-memory-producer`) and its nextest `threads-required` override. Restoring code deleted there counts as reuse,
   not new machinery. The refused-Delete test can use the Delete refusal terminal hosts already have (Delete guarded on
   the agent having ended).
5. SPEC_impl.md's known-gaps sentence for these cases (around its lines 1874 to 1878 at planning time) goes, and the
   supervisor's refusal text matches SPEC.md's "fails clearly with an error naming it".
6. A changelog fragment per root `AGENTS.md`, "Changelog fragments", and the last code PR removes this plan's TODO.md
   entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page:

- **A zero-size request is refused with a clear message,** as the TODO entry says. Rejected: keeping today's silent
  ignore and pinning it with a test.
- **Plan it, assuming it is not very complicated** (the maintainer's condition above; see Fallbacks).
- **The folder check lives in the supervisor,** so the terminal host does not change (planner choice, stated on the
  page).
- **Release note:** none needed (see the section at the end).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the explanation and instruction above, and the Decisions.

**Binding repository constraints:** SPEC.md's rule that a launch in an unusable folder "fails clearly with an error
naming it"; `crates/terminal-host/SPEC.md` (sizes from 1 to 10000; its "Change policy"); SPEC_impl.md's known gaps; root
`AGENTS.md`, "Code rules", "Changelog fragments", "Conventional Commits", "Tests and validation";
`.agents/test-authoring.md`; `docs/test-sleep-check.md` when changing Rust tests (run
`python -B scripts/check-test-sleeps.py` as it says).

**Planner proposals** (from reading the code at planning time; follow the code where it differs, logged as a DECISION):

- Create, restart and tab open run `ensure_cwd_usable` first (`crates/farhelm-supervisor/src/service/core.rs`), which
  reads only the folder's metadata, so a folder the user can see but not enter passes. Extend it with an access check
  for search permission (`rustix::fs::access` with `Access::EXEC_OK`; `rustix` is already a supervisor dependency, and
  the terminal host's `is_executable` uses the same kind of check). Opening the folder as a directory is the wrong test:
  it needs read permission, and the lost tests' own fixture (mode 0o600) opens fine but cannot be entered. Today the
  terminal host then fails to enter it and reports `starting {program}: {errno}` (`crates/terminal-host/src/spawn.rs`),
  naming `systemd-run` because the supervisor puts `systemd-run --user --scope` in front of every launch.
- A failed create is rolled back today, so nothing is left running.
- The 0x0 refusal is the supervisor's own terminal-host client refusing the size (`HostTerminal::resize` in
  `hosts/terminal.rs`, `ResizeRefused::Invalid`); it never reaches the terminal host. The attach handler
  (`service/handlers.rs`) runs that resize after any incumbent viewer has been detached and only logs the failure, so
  the size check moves to the attach's preparation, before anything is displaced.
- The memory test's premise (tmux's `pause-after`, a 7-second allowance) was tmux-specific and needs restating for
  terminal hosts; the upload-ack test's own doc said its measurement does not carry over to the grid stream as it was.
  Port both if restoring their deleted helpers plus a restated premise is enough.

## Outline

1. `plan/unusable-folder-refusals/01-refuse-unenterable-folder`, `fix:`: criterion 1 and 2, the first two tests, the
   SPEC_impl.md edit for the folder half, and the fragment.
2. `plan/unusable-folder-refusals/02-refuse-zero-size`, `fix:`: criterion 3 and the zero-size test, the rest of the
   SPEC_impl.md edit, and its own fragment (or an edit of PR 1's, if both describe one user-facing change; your call).
3. `plan/unusable-folder-refusals/03-port-lost-tests`, `test:`: the memory and upload tests, and the TODO.md entry
   removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the supervisor, the ported tests and the supervisor's launch, tab and attach tests through
`python3 scripts/record-test-run.py`, `python -B scripts/check-test-sleeps.py`, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

- **The maintainer asked to be told if this turns out complicated.** Restoring the helpers deleted in `647f0227b` is
  fine. If porting the memory test or the upload-ack test needs new test machinery beyond those restored helpers and a
  restated premise (a new way to measure memory, or a new hook into how terminal output is streamed), do not build it:
  finish everything else, leave that test out, and say so in the report's things you should know, with what it would
  take.
- If refusing a zero-size attach would break a client the repository still has (a tool or test that attaches before it
  knows its size), fix that client to send its real size if that is small; otherwise keep today's behavior for attach
  too, and say so in the report.

Decisions to log: how "can be entered" is checked, the refusal wording, how a zero-size request is refused and where the
message appears, how each lost test was ported (or why it was left out), every test changed and why, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-unusable-folder-refusals-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-unusable-folder-refusals-watchdog.json` beside it. Resolve
both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation".

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
  `plan/unusable-folder-refusals/<nn>-<short-name>`, as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). This plan's `## Release note` section is the maintainer's entry for the release, not an instruction to
  you: each PR still leaves the fragment its type calls for, describing what that PR built, and where what got built
  departs from the release note (a renamed control, a part left out, a limitation it does not mention), the fragment's
  context paragraphs say how (`plans/.agents/executing.md`, "Executing one plan", step 8).
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).
- The last code PR of the stack removes this plan's TODO.md entry.

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
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires.

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
meeting the acceptance criteria above, each having passed its review gate, and the last code PR removing this plan's
TODO.md entry. Open, not merged. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close
the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue
script, write a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.

## Release note

No release note needed.
