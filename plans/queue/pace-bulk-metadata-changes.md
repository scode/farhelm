# Keep the window connected when many marked sessions change at once

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Pace metadata changes to marked sessions.** Pin, Unpin, Mute, Unmute, Mark read and Mark unread can issue a burst
> from the desktop's marked menu. The helm closes its UI channel when more than 64 commands are pending, leaving the
> window reconnecting with a partly applied batch. Use the existing reply-paced Stop/Delete and group-move mechanisms as
> guides. Keep each eligible row's drawn write, acceptance-time refusal if busy, row-local failures, and cancellation on
> reconnect. Cover a batch larger than the channel limit and show that rows keep making progress after a refusal. Plan:
> `plans/queue/pace-bulk-metadata-changes.md`.

The entry asks for pacing. The maintainer decided against pacing (Decisions below): the fix is to raise the helm's
per-window limit on queued commands, so the GUI's behavior does not change at all.

### Where it stands (from planning research at main b34ceee9f)

In product terms: mark about 66 or more sessions and choose Pin, Unpin, Mute, Unmute, Mark read or Mark unread from the
marked bar's ⋯ menu. The desktop app sends one command per eligible session, all at once. The helm keeps at most 64
commands waiting behind the one it is running, and when the next arrives it hangs up on the window. The user sees
"Reconnecting to the helm…" and the connection-lost banner, marks are gone, and only some sessions got the change. The
listing cap is 500 rows, so this happens at fleet sizes the product supports, and SPEC.md, "Losing the connection to the
helm" (about line 5491), expects a reset only when the connection actually breaks.

- The limit: `MAX_QUEUED_COMMANDS = 64` in `crates/farhelm-helm/src/ui_channel.rs` (about lines 124-130), checked in the
  channel's `run` loop (about 1039-1046): a command arriving while that many already wait behind the one in flight
  closes the channel and drops the queue. Heartbeats (pings) and the in-flight command are not counted. Its doc comment
  says "A real GUI never comes close: its watchdog resets the window long before", which bulk marking falsified. The
  value matches the channel's other buffers (`OUTGOING_QUEUE`, `INCOMING_QUEUE`, about 116-122), and no commit message,
  spec or `lore/` file records a reason for 64. `MAX_CLIENT_MESSAGE` (about line 92) bounds one message at 4 MiB.
- The helm test `a_gui_that_floods_commands_behind_a_stuck_one_is_disconnected`
  (`crates/farhelm-helm/src/ui_channel_tests.rs`, about 603-650) holds the first command, queues
  `1..=MAX_QUEUED_COMMANDS`, proves the queue accepted them with a heartbeat, then sends one more and expects the close.
  Its loop follows the constant, but the overflow command uses the literal id 999, which falls inside the queued range
  once the limit is 1000. Its docstring repeats the old "without bound" reasoning.
- `LIST_SESSIONS_CAP = 500` (`crates/farhelm-proto/src/lib.rs`, about line 332); its doc says the helm applies the same
  cap to its merged view (about 329-331), so the desktop app never lists more than 500 rows.
- Why the GUI's bursts are bounded by the row cap: the window's operation store allows one request per key at a time
  (`crates/gui/farhelm-gui/src/operations.rs`, module docs about 14-26, admission about 883-893), so a second action on
  a row whose request is still pending is refused on that row instead of sent. The marked menu's `apply_metadata`
  (`crates/gui/farhelm-gui/src/session_list/marking.rs`, about 841-924, the loop about 897-921) sends each eligible
  row's own write straight through `pins::send_pin`, `mute::send` or `seen::send`. Commands the window has outstanding
  for rows therefore never exceed the listed rows plus the open session. Hosts, templates and preferences add a handful.
- The window's side deliberately has no cap: `crates/farhelm-helm-client/src/channel.rs` (about 507-508), "No local
  queue or cap: the helm closes a channel whose queue overflows, and that is the same break as any other." That stays
  true.
- Other bulk paths are already paced: group moves through `MarkedMoves`
  (`crates/gui/farhelm-gui/src/session_list/groups.rs`, about 541-588, whose doc says one write at a time keeps a large
  selection within the helm's channel bound, which stays true), and bulk Stop and Delete one request at a time per host
  (`session_list/bulk.rs`). Neither changes.
- Neither SPEC.md nor SPEC_impl.md states the limit (searched for the constant and the number); SPEC_impl.md, "One
  failure path: the reset" (about 453-466) describes the watchdog, which times only the head command, so a long queue of
  quick commands never trips it.
- The cost the raised limit accepts: commands stay strictly first in, first out, and after every command the helm
  re-reads every pushed collection, the whole session list included, before replying (`Channel::changes`,
  `ui_channel.rs`, about 807-829). A 500-row Mute therefore queues 500 writes and 500 re-projections, and a click made
  right after it (a Rename, a Stop, a launch) waits behind all of them: unmeasured, probably a second or a few at 500
  rows, unnoticeable at tens. The rows draw as pending meanwhile, and the watchdog is not at risk because it times only
  the head command. This is the tradeoff the maintainer's decision accepts; the report's "things you should know" says
  so.
- GUI tests talk to the fake helm (`crates/farhelm-helm-client/src/fake.rs`), which enforces no queue limit, so no GUI
  test can observe the helm's bound; the helm's own flood test is what pins it. The GUI's side is already covered:
  `metadata_writes_go_once_per_eligible_row_and_clear_the_marks` (`session_list/marking_tests.rs`, about line 777) shows
  one write per eligible row, and `a_busy_row_gets_the_not_started_line_until_its_next_operation` (about 858) shows a
  busy row refused while the others go on.

### Acceptance criteria

1. `MAX_QUEUED_COMMANDS` is derived from `LIST_SESSIONS_CAP` (twice it, 1000 today) rather than a bare number, and the
   helm still closes a channel whose queue goes past it, exactly as now.
2. The constant's doc comment says why the value is safe and enough: the operation store's one request per session
   bounds the GUI's bulk bursts by the row cap; a broken window could still queue that many messages, which is accepted
   because the peer is the app's own window; and code that queues commands not tied to a row (a loop over templates or
   hosts, say) must keep this bound in mind, since nothing else limits it. No invented history: it does not claim a
   reason for the old 64.
3. The helm's flood test still proves that a full queue is accepted and one more closes the channel, with its overflow
   command's id outside the queued range and its docstring matching the new reasoning. That test, now queueing
   `2 * LIST_SESSIONS_CAP` commands behind a stuck one, is the evidence for a batch larger than the old limit; with the
   derived constant and the two existing GUI tests named above it is the whole coverage. No new desktop-app test is
   added.
4. The GUI's behavior is unchanged: no pacing, no batch command, no byte cap, no change to `MarkedMoves` or to bulk Stop
   and Delete pacing. Each row's drawn write, the refusal of busy rows at acceptance, row-local failures and
   cancellation on reconnect come along as they are.
5. The report's "things you should know" states the accepted cost: a click made right after a large batch (up to 500
   rows) waits behind it.
6. The PR carries a changelog fragment and removes the TODO entry.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Raise the limit; do not pace and do not add a batch command.** In the maintainer's words: "Yeah, being super careful
  here is just completely over-optimizing for something that is very unlikely to be a real issue. Let's just keep it
  freaking simple: have a limit that is large enough that it doesn't ruin the user experience or cause complexity in
  other areas. The probability of actually having memory issues here is very small until we make some kind of change to
  the UI where some kind of unbounded or large accumulation of commands could occur." So: no bound on queued bytes, no
  pacing of metadata writes, no batch command, no change to what the GUI does.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, narrowed by the decisions above.

**Binding repository constraints:**

- SPEC.md, "Losing the connection to the helm" (a reset only when the connection breaks) and "Session list" (metadata
  choices "reuse the ordinary writes and row-local errors, and clear marks once their eligible writes have been
  dispatched", about 2379-2380), which this plan keeps as they are.
- SPEC_impl.md, "One failure path: the reset", unchanged.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and its "Instructions by
  path" for every path you touch.

**Planner proposals:**

- The value `2 * LIST_SESSIONS_CAP`: it covers a full listing's burst plus the open session and the handful of other
  keys with room to spare, grows with the cap if the cap grows, and names its reason in the code. `farhelm-helm` already
  depends on `farhelm-proto`.
- `MarkedMoves` (group move pacing) stays as it is: once the limit is raised it could go back to plain sends, but that
  churns tested code for no user-visible gain.
- No new desktop-app test: the fake helm has no queue limit, so a GUI test with more than 64 rows would pass on today's
  code and prove nothing the existing GUI tests do not. The TODO's "cover a batch larger than the channel limit" was
  written for pacing; under the maintainer's decision the revised flood test covers it, in the one layer that enforces
  the limit.
- Leave the client's "No local queue or cap" comment and `MarkedMoves`' doc comment alone: both stay true.
- Fragment, `kind: fixed`, along the lines of: "Pinning, muting or marking read or unread a large number of marked
  sessions at once no longer makes the window reconnect with only some of them changed."

## Outline

1. `plan/pace-bulk-metadata-changes/01-raise-queued-command-limit`: the constant and its doc comment, the flood test's
   overflow id and docstring, a changelog fragment, and the TODO entry's removal. `fix:` (for example
   `fix: keep the window connected when many marked sessions change at once`), with a fragment.

## Fallbacks

- If you find a GUI path that can queue commands not tied to a row in numbers approaching the new limit (a loop over
  templates, hosts or preferences, say), do not add pacing or a cap for it in this plan: name it in the doc comment
  where it fits, log it, and report it as a follow-up.
- If SPEC.md or SPEC_impl.md turns out to state the limit after all, update that text in the same PR and log it.
- A reviewer finding that asks for a byte bound, pacing, or a batch command is declined by decision; log it as such. So
  is one that asks for a test of the burst through the desktop app or against a real helm.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-pace-bulk-metadata-changes-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-pace-bulk-metadata-changes-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits").

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks:
  `plan/pace-bulk-metadata-changes/01-raise-queued-command-limit`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Pace metadata changes to marked sessions"), as
  root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments"). A later PR of this stack that amends work an earlier PR of it introduced edits that PR's fragment rather
  than adding a second one. This plan's `## Release note` section is the maintainer's answer for the release's
  highlights, not an instruction to you: each PR still leaves the fragment its type calls for, describing in user-facing
  words what was built, and saying in its context paragraphs where what was built departs from the release note
  (`plans/.agents/executing.md`, "Executing one plan", step 8). Never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
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

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
