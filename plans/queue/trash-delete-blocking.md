# UI: show real progress while the trash dialog deletes checkouts

Written against main at 36c8d7b0 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Fix trash deletion silently blocking the UI.** Deleting a session to trash can silently block everything the user
> does — including closing the window — while it progresses, sometimes slowly, with no progress indication.

The maintainer's clarification on the planning page: the flow is **permanent deletion from the trash dialog**. "You
click delete and you can't dismiss the dialog with the trash list so you can't do any work, and there is no sign of any
visible progress and I've had this take like 30ish seconds multiple times. I'm open to a simple fix where we at least
just give a very clear indicator that something is happening, we don't _need_ to make it run in the background etc,
let's keep it simple." On the indicator: "something like an upside down trash can shaking up and down icon as the number
goes down."

How it works today, from a read of main at 36c8d7b0 (line numbers approximate):

- UI, `crates/farhelm-ui/src/list/trash.rs`: `TrashControl` (around 453) stays mounted under the list. `dispatch`
  (around 588-686) handles an entry's `delete`, a host's `delete all on <host>` and the header's `delete all`. It takes
  the page-wide operation claim (around 617), sets `deletion = Some(DeletionRun { pending, total })`, and sends **one
  request per host carrying all that host's ids** (`api::empty_checkout_trash`, `crates/farhelm-ui/src/api.rs` around
  3487-3502, `POST /api/checkout-trash/delete`). The claim is released when every host has answered.
- The only progress is the status line "deleting…" (one host) or "deleting on N of M hosts…", plus a per-host
  "deleting…" (render around 800-960). Close and Escape do nothing while a run is in flight, which SPEC.md requires
  (around 1152-1154).
- The request uses the shared 60 s `REQUEST_TIMEOUT` (api.rs around 1587, 1650-1655). When it expires, the dialog says
  "The cleanup reply could not be confirmed…" and releases the claim, while the host carries on.
- Helm, `crates/farhelm-helm/src/sessions.rs` (around 546-561), checks the host incarnation and forwards. The supervisor
  (`crates/farhelm-supervisor/src/service/core.rs` around 5080-5097, `checkout_trash::delete` around 368-470) takes the
  host's directory admission and removes each checkout recursively, file by file. Its time grows with the checkout's
  size. Per-record problems come back as issues, and the rest of the batch continues.

### Acceptance criteria

The dialog stays modal and blocking, as today. What changes is that the user can see it working:

- **One checkout at a time per host.** For an entry delete, a host's delete all and the global delete all, the page
  sends one request per checkout, in sequence for each host, with hosts running in parallel. No protocol, helm or
  supervisor change.
- **The trash icon animates while a run is in flight.** In the dialog header and on the trash button, the icon turns
  upside down and bobs up and down. No word is added to the trash button, because it would not fit. The bob follows
  SPEC_impl.md's looping-indicator rule (around 480-493): a stepped timing function at no more than 10 steps per second
  across the cycle, and static (still upside down) under Reduce Motion. The spinner on the current entry reuses the
  existing reduced-motion-aware spinner.
- **The count ticks down.** The header's checkout count and size go down as each checkout is deleted, and so does the
  trash button's count.
- **Per-entry state.** The entry being deleted on each host shows a spinner and the seconds elapsed since its request
  started. Entries already deleted disappear. Entries still queued say "waiting". Each host's header shows how many of
  its selected checkouts are deleted so far ("1 of 3 deleted").
- **Status line.** The existing status region (kept mounted so screen readers announce changes) states how many
  checkouts are left, rather than only a host count.
- **Close explains itself.** Close and Escape stay unavailable until the run finishes, and Close's tooltip says it
  becomes available when the delete finishes.
- **A longer wait.** Trash delete requests wait up to 15 minutes for the host's answer instead of the shared 60 s. Other
  requests keep 60 s.
- **Failures keep today's meaning.** A per-record issue appears where delete issues appear today (the host's issue list
  at the bottom of its section), and the host moves on to its next checkout. Today that list is replaced on each reply;
  with one reply per checkout it must accumulate within a run, so earlier issues are not wiped. A lost or unconfirmed
  reply stops that host's remaining queue and says some folders may already be deleted, as today. Other hosts carry on.
  The listing is refetched once when the whole run ends, as today; no mid-run refetch.
- SPEC.md's trash dialog paragraph (around 1141-1154) and SPEC_impl.md's "Archived-checkout trash" (around 2620-2681)
  describe the per-checkout dispatch, the progress shown, and the longer wait.
- Browser tests in `e2e/tests/trash-dialog.spec.ts`, holding the per-checkout requests on promises as the file already
  does for the batch request:
  - while a request is held, the current entry shows its spinner, the others show "waiting", the icon is in its animated
    state, Close is disabled with its tooltip, and the counts are unchanged;
  - releasing a request removes that entry and lowers the header count, the host count and the trash button count, and
    the next entry becomes current;
  - the elapsed counter advances without real waiting (Playwright's clock control, or whatever the suite already uses);
  - a per-record issue mid-run shows in the host's issue list, stays there after later replies, and the run continues; a
    failed request stops only that host;
  - the existing progress assertions written for today's per-host wording are rewritten for the new status line;
  - with two hosts, both progress at once.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **Only the trash dialog's permanent deletion.** Deleting a session, which moves its checkout to the trash, is out of
  scope.
- **Keep it simple: the dialog stays blocking.** The maintainer considered a closable dialog and deleting in the
  background, and preferred clear progress in a blocking dialog. No background removal, no closable dialog, and no
  change to how the host queues work behind the delete.
- **The progress design as mocked on the planning page**, described in words under Acceptance criteria above: an
  upside-down, bobbing trash icon in the dialog header and on the trash button; counts that tick down; a spinner with
  elapsed seconds on the current entry; finished entries disappear; queued ones say "waiting"; per-host "N of M
  deleted"; Close's tooltip.
- **Per-checkout progress comes from per-checkout requests**, not a protocol change. Progress inside one checkout (files
  or bytes) is out of scope.
- **Trash delete requests wait up to 15 minutes.**
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the maintainer's words quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's managed checkouts and trash dialog (around 1141-1171) and its modal dialog
rule (around 1308-1309); SPEC_impl.md's "Archived-checkout trash" (around 2620-2681) and "Who owns an accepted action"
(around 21-58); the operation claim's contract in `crates/farhelm-ui/src/ops.rs` (module docs); SPEC_impl.md's
looping-indicator rule (around 480-493), enforced by `crates/farhelm-ui/js-tests/app-css-animations.test.js`;
`docs/desktop-web-triage.md`; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments",
"TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Keep the run's state in `TrashControl`, which already outlives the dialog's render, and extend `DeletionRun` from a
  set of pending hosts to per-host queues with the current checkout and its start time.
- Freeze the targets at confirmation, as today: each per-checkout request carries the incarnation and id frozen then,
  and the helm repeats the incarnation check anyway.
- Give the trash delete call its own deadline through the existing helper for a caller-chosen request deadline in
  `api.rs`, rather than changing the shared `REQUEST_TIMEOUT`.
- Draw the icon with the existing `TrashIcon` component and a CSS class for the flipped, bobbing state, so the trash
  button and the header share it.
- Only checkouts the run actually deletes get the spinner and "waiting" states; rows that are not part of the run (for
  example diagnostic rows that cannot be deleted) keep their current look.
- The elapsed counter ticks once a second from a client-side timer, and only while a request is in flight.

## Outline

One PR, or two if it grows past a comfortable review:

1. `plan/trash-delete-blocking/01-per-checkout-progress`: per-checkout dispatch, per-entry and per-host state, counts,
   the animated icon, the status line, Close's tooltip, the 15-minute wait, SPEC.md and SPEC_impl.md, the browser tests,
   the changelog fragment and the TODO.md removal. If it is too large, split the dispatch and the longer wait (with
   their tests) from the visual progress, keeping each PR complete on its own.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-ui`, the CSS animation test (`crates/farhelm-ui/js-tests/app-css-animations.test.js`),
`python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares, and, through
`python3 scripts/record-test-run.py`, `e2e/tests/trash-dialog.spec.ts` and any other spec that clicks the trash button,
on both engines.

## Fallbacks

Examples of departures that need the scope reassessment: a closable dialog, background removal, a new progress protocol
between host, helm and page, changing the page-wide claim, or touching session Delete.

Decisions to log: the run state's shape, the failure semantics where today's batch behavior and per-checkout requests
differ, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-trash-delete-blocking-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/trash-delete-blocking/<nn>-<short-name>`,
  as the outline names them.
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
