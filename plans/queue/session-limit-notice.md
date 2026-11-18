# Tell the user when the 500-session ceiling is hit

Written against main at 04631d86 on 2026-11-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, and this plan changes what they say about the session list's cap.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Revisit the 500-session cap per host.** A host with more than 500 sessions cuts its list, and the helm never sees
> the cut sessions. Work out exactly what the limit is and the full set of implications when it is hit. One known one:
> if the open session is among the cut ones, the app cannot tell that it was deleted.

The planner worked out the limit and its implications (below). The maintainer then decided (2026-11-18): keep the
ceiling, add no special handling for it, and instead tell the user plainly through the navigation rail's notification
bell when it is hit, asking for feedback; making the list scale goes into a "Maybe later" TODO entry. In their words:

> keep the celing but use the notificatio bar to tell the user that the ceiling was reached. (whether it's the
> supervisor local ceiling or the helm limit), and ask the user to submit feedback if they actually want this many
> sessions in real use, or if a bug caused this to occur. do not add complexity to handle it gracefully.

> i meant the actual notificatio bell we have in the navigation rail - the farhelm global notification mechanism.

### What the limit is (the planner's findings at 04631d86)

- `LIST_SESSIONS_CAP = 500` (`crates/farhelm-proto/src/lib.rs`). Its doc and SPEC.md ("The list is served and rendered
  WHOLE …") say why: Farhelm is built for tens of sessions on a few hosts, lists are served and drawn whole with no
  paging, and the cap is a safety ceiling an order of magnitude past that, meant to be hit only when something has gone
  wrong, which also keeps a reply under the frame limit.
- A supervisor sorts its sessions newest-created first and replies with 500, marked `truncated`
  (`crates/farhelm-supervisor/src/service/listing.rs`). A session the cut leaves out, running or not, is unreachable
  from the app: no attach, Stop, Delete or Rename, no notifications, waiting cues or ntfy alerts, and the quick switcher
  cannot find it.
- The helm refuses a reply over 500 (`manager.rs`), evicts its oldest cached row when a create lands on a host at the
  cap (in its store and connection manager; `session_cache.rs` holds the victim rule), and cuts its merged view across
  all hosts to 500 (`aggregate.rs`). Three hosts with 200 sessions each therefore show a cut list.
- While any listing is cut the app cannot tell that the open session was deleted; the list's count says it "could not
  read to the end".

### Acceptance criteria

1. **A host's ceiling.** When a host's session listing comes back cut at its ceiling, the host/app notification bell in
   the navigation rail (SPEC.md, "Notifications about a host or the whole app have a separate bell …") gets a notice for
   that host. It names the host, says it has more than 500 sessions so the list shows only its newest 500 and the others
   cannot be reached from the app, says Farhelm is built for tens of sessions, and asks the user to send feedback if
   they want this many on purpose or think a bug caused it. A **Send feedback** action opens the existing feedback
   dialog, through the same handoff the Welcome card already uses to open it. Raise and resolve it where the helm
   already stores that a host's list was cut (`hosts.cache_truncated`, written with each listing), including the
   identity reset that clears that flag. Hosts that report no identity keep that flag only in memory; skipping them is
   acceptable, and logged.
2. **The helm's ceiling.** When the helm's whole unfiltered fleet exceeds 500 sessions, so its merged list across hosts
   is cut, the bell gets one app-wide notice with the same request, saying the list across your hosts shows only 500
   sessions. A filter's result never raises it. The helm already counts the whole fleet before any filter; evaluate the
   condition wherever that count changes (a listing, a session delete, a host removal), not only where listings are
   recorded. This notice is independent of the host notices: both can show at once.
3. **Bell behavior, unchanged.** The notices behave like the bell's existing kind: unread until shown, marked read when
   the panel closes, keep their identity and read state while the condition persists so they never re-alert merely
   because the next listing is cut again, and become resolved (quiet, greyed, removable with Clear resolved) once the
   host's listing, or the merged list, is no longer cut. Removing a host removes its notice. When a resolved notice's
   condition comes back before the user cleared it, it follows the existing kind's rule for a recurrence (reopening
   under its identity); say so in SPEC.md and test it.
4. **No other change to the cut.** Which sessions are kept, the eviction on create, the helm's merged cut, the number
   500 and the protocol stay as they are. The list's existing "could not read to the end" wording stays.
5. **Specs.** SPEC.md's bell paragraphs list the new kinds (and drop "No toast or new notification kind ships with this
   behavior", which described the first kind's landing), and the session list's cap paragraph says the bell reports a
   hit ceiling and asks for feedback. SPEC_impl.md says where the helm raises and resolves them.
6. **Tests:** the helm raises a host notice when a listing is cut and resolves it when it is not; the app-wide notice
   likewise for the merged cut; neither re-alerts while the condition persists; the GUI draws the new kinds and Send
   feedback opens the feedback dialog.
7. The planning PR added a "Maybe later" TODO entry for making the session list scale past the ceiling; this plan does
   not implement any of it.
8. A changelog fragment per root `AGENTS.md`, "Changelog fragments", if a PR's type calls for one, and the last PR
   removes this plan's TODO.md entry.

## Decisions already made

Settled with the maintainer on the 2026-11-18 planning page:

- **Keep the ceiling and handle nothing more gracefully** (the quotes above). Rejected: keeping running, waiting and
  unread sessions through the cut; removing the merged cut; raising the number; a bulk "delete old ended sessions"
  action; removing the cap altogether.
- **The warning goes to the navigation rail's notification bell**, the global notification mechanism, for both the
  host's ceiling and the helm's. Marking read and clearing are the bell's own.
- **A "Maybe later" TODO entry** records making the list scale.
- **No release note** (see Release note).
- **Reviewer:** a single fresh-context agent on gpt-6.2-sol at high effort (see Review gate).
- **Builds and tests on the executing machine,** not the boat sandboxes.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Scheduled as pending**, with no dependency on another plan.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry, the quotes, and the Decisions above.

**Binding repository constraints:** SPEC.md's session list cap paragraph and the rail bell's paragraphs, SPEC_impl.md's
UI channel and notice passages; root `AGENTS.md`, "Code rules", "Changelog fragments", "Conventional Commits", "Tests
and validation"; `.agents/test-authoring.md`.

**Planner proposals** (from reading the code at 04631d86; follow the code where it differs, logged as a DECISION):

- The bell's existing kind (trashed checkouts whose background cleanup failed) is the pattern to extend: its notices are
  already raised by the helm and kept in the helm's database, with an identity that survives recurrence, a resolution,
  and actions. Extend that store rather than adding a second notice path. It is built only for that kind, so expect, as
  part of the agreed work and not a departure: a schema migration (the `notices` table accepts only the trash kind and
  requires a host), an optional host for the app-wide notice in the shared notice type and in the GUI panel, and a kind
  filter on the trash recorder, which today resolves every other notice on the same host and would otherwise resolve
  ceiling notices. One kind with an optional host or two kinds: yours to choose, logged.
- The trash flow's request-ordering machinery is not needed here; the cut flag is written in the same store write as the
  listing.
- No protocol change between helm and supervisor is expected; `truncated` is already on the wire.

## Outline

1. `plan/session-limit-notice/01-ceiling-notices`: criteria 1 to 6, SPEC.md and SPEC_impl.md, tests. `feat:` (users
   newly see a notice), with a fragment. Removes this plan's TODO.md entry. Split the helm side and the GUI side into
   two PRs if the one grows past a comfortable review size.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `cargo fmt` check, clippy with `--all-targets` on
the changed crates, the helm's notice and listing unit tests and the GUI crate's rail bell tests through
`python3 scripts/record-test-run.py`, the GPUI goldens if the bell panel's drawing changes, and
`python3 releasing/check-changelog.py format`.

## Fallbacks

- Generalize the bell's notice store just far enough for these notices and log what changed; a second notice store is a
  departure (run the scope reassessment).
- If raising a notice needs a protocol change between helm and supervisor, that is a departure: run the scope
  reassessment.

Decisions to log: the notices' kinds and identities, where the app-wide condition is evaluated, the schema change, every
test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-session-limit-notice-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-session-limit-notice-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

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

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/session-limit-notice/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one (root `AGENTS.md`, "Changelog
  fragments").
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
