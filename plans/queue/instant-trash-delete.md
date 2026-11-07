# Trash deletes finish at once; hosts free the space in the background

Written against main at 66e77cf4 on 2026-11-07. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Deleting checkouts from the trash is far too slow.** Each delete costs a multi-second round trip and blocks its
> caller. Speed it up by doing the deletes concurrently, move them into the background so the UI is not blocked, or
> both.

What the planning research found (line numbers approximate, main at 66e77cf4):

- **Where the time goes.** The supervisor logs per-step timings for every permanent trash deletion (`checkout_trash.rs`,
  "permanent checkout trash deletion finished"). A real run on a Linux host deleted 73 checkouts in 54 seconds: 50.6 s
  of it was the file removal itself, about 47 ms per checkout was everything else (page, helm, network, the lock,
  database and safety checks), and the host-wide lock was free throughout (`admission_ms=0`). Removal cost about 130 µs
  per file or directory, against about 33 µs for `rm -rf` on the same disk, and it follows the number of entries, not
  bytes: checkouts small in megabytes held 30,000-50,000 entries and took 3-7 s each. Sending deletes concurrently from
  the page would not help, because the host serializes them behind its lock.
- **UI** (`crates/farhelm-ui/src/list/trash.rs`): `dispatch` (around 660-834) serves an entry's Delete, a host's Delete
  all and the global Delete all. It takes the page-wide operation claim (`gate.claim_guard()`, around 691; `ops.rs`
  module docs: a held claim disables every write action in the window), spawns one task per host, and sends one request
  per checkout, sequentially per host (around 731-762). The dialog is modal and Close, the header close and Escape are
  unavailable during a run. The plan `trash-delete-blocking` (landed 2026-10-25) built that per-checkout progress: an
  upside-down bobbing trash icon on the trash button and the dialog header, counts ticking down, a spinner and elapsed
  seconds on the current entry, "waiting" on queued ones, per-host "N of M deleted". `api.rs` `empty_checkout_trash`
  (around 4066-4089) uses `TRASH_DELETE_TIMEOUT`, 15 minutes (around 2004).
- **Trash count.** TrashControl re-reads `/api/checkout-trash/list` only on load, host signature change, `revision`
  bumps, dialog open and run completion (around 561-607); SPEC_impl.md (around 4191) says no new poll runs.
- **Helm** (`crates/farhelm-helm/src/sessions.rs` around 546-562) checks the host incarnation and forwards
  `ControlMsg::DeleteCheckoutTrash { ids }` (the protocol already takes a list, capped by `DELETE_ID_CAP`).
- **Supervisor**: `service/core.rs` `delete_checkout_trash` (around 5984-6008) takes the host-wide
  `working_copy_operations` lock (also taken by session create, restart and session Delete), then
  `checkout_trash::delete_timed` (around 514-676): per ID, it counts sessions using it, `open_archive` (identity checks
  against the recorded root, inode and birth time, and against every session path), `remove_opened_tree_with`
  (file-by-file removal through confined calls that never follow a link or enter a swapped-in folder), then
  `forget_retired` drops the record. Trash folders live under each checkout root (`<root>/.farhelm-trash`, SPEC.md
  around 1432-1435), and the working-copy records are the only map of those roots. The spare discard path
  (`spare_checkouts.rs` around 596-620) already renames to a private name and then removes by handle.
- **Specs:** SPEC.md's managed-checkout lifecycle and trash dialog (around 1445-1475, and 1452's "unavailable under the
  same host-listing and operation guards"); SPEC_impl.md "Archived-checkout trash" (around 4129-4215, including
  "Directory admission stays held through the blocking operation" and the per-checkout dispatch around 4199-4211).
- **Tests:** supervisor unit tests in `checkout_trash.rs` (around 924-1600: completion logging, legacy migration, mount
  subtrees, bookkeeping failure, ancestor replacement, symlinks, sessions inside archives); helm
  `checkout_trash_routes_pass_host_results_and_fence_stale_deletion` (`sessions_tests.rs`); proto
  `checkout_trash_roundtrips_and_correlates`; UI `confirmation_answer_deletes_only_its_unchanged_selection`
  (`trash.rs`); browser `e2e/tests/trash-dialog.spec.ts` (mocks `/api/checkout-trash/delete`; around 332 asserts today's
  one-at-a-time progress).

### Acceptance criteria

Host:

- **A delete answers without removing the files.** For each ID, the host verifies the checkout exactly as today (same
  refusals, same identity checks), then renames its folder to a reserved "being removed" name inside the same
  `.farhelm-trash` folder and answers. The rename is the only filesystem work under the host-wide lock.
- **The record stays until the files are gone.** The working-copy record is kept, pointed at the reserved name, and the
  trash listing no longer shows it. `forget_retired` runs when its removal finishes. This is how a restarted host finds
  leftovers; no new table or record file.
- **A background remover.** One supervisor-owned task removes those folders one at a time, with the existing confined
  removal, without holding `working_copy_operations` and without using the browse worker pool. Launches, restarts and
  session Deletes never wait for it.
- **After a restart** the host resumes removing whatever records still point at reserved names.
- **A folder it cannot remove** (files owned by a container's root user, say) is logged as a warning, keeps its record
  and its reserved name, and is retried at the next start only (no retry loop in a running supervisor). Nothing else
  gets stuck or breaks because of it.
- **The host reports removal in progress:** an optional field on the trash listing reply, true while an attempt is
  actually running in this supervisor (not merely while leftovers exist). An older supervisor's listing reads as not
  removing.

UI:

- **Confirmed bulk deletes close the dialog at once.** Confirming a host's Delete all or the global Delete all closes
  the dialog immediately; the deletes go out in the background, one request per host carrying all of that host's
  selected IDs (the per-checkout queue is no longer needed). An entry's own Delete keeps the dialog open, and the entry
  leaves when its reply arrives.
- **The trash icon shows emptying.** The trash button's icon is in its animated "being emptied" state while this page's
  delete requests are in flight or any host's listing says removal is in progress, and its tooltip explains it (for
  example "Emptying the trash: freeing space on <host>"). Reuse the existing upside-down bobbing icon and its
  reduced-motion handling. While a host's last listing said removing, the page re-reads that host's listing on a short
  timer until it says otherwise; no poll runs otherwise.
- **No page-wide block.** Deletes no longer hold the page-wide operation claim; only the trash control's own busy state
  keeps a second run from starting while this page's requests are in flight.
- **Refusals stay findable.** A refused or failed checkout stays in the trash; its issue shows when the dialog is next
  opened (no longer cleared on open), and while one is pending the trash button's tooltip says some checkouts could not
  be deleted and to open the trash to see why. No new notice surface.
- **The per-checkout progress UI goes:** the per-entry spinner, elapsed seconds, "waiting", "N of M deleted", the header
  icon's run state and the Close tooltip about a run in progress, with their tests.
- **Older hosts keep working.** The 15-minute delete deadline stays, because a host on an older supervisor still removes
  inside the request.

Specs, docs and the rest:

- SPEC.md (around 1445-1475) and SPEC_impl.md "Archived-checkout trash" (around 4129-4215, and the no-poll sentence
  around 4191) describe the new behavior; the docs page about the trash, if it describes the dialog's progress, matches.
- Tests: supervisor tests for the rename under the lock and the reply before removal, the record kept and hidden from
  the listing, the background removal and its `forget_retired`, the resume after a restart, an unremovable folder logged
  and left for the next start, and the listing's in-progress field; helm and proto tests for the new field; browser
  tests for the dialog closing on a confirmed bulk delete, the entry delete keeping it open, the animated icon and its
  tooltip while requests are in flight and while a listing says removing, the end of the animation, and a refusal shown
  on the next open, on Chromium and WebKit.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: changed`),
  saying that disk space comes back shortly after the trash empties rather than at once.
- The last code PR removes the TODO.md entry quoted above. The separate Near term entry "Show the user when the trash
  cannot finish freeing space" stays: it is follow-up work, not part of this plan.

## Decisions already made

Settled with the maintainer on 2026-11-07, on the planning page, after the research above was shown there:

- **Approach: delete instantly, free the space in the background** (the page's option A, not the faster-removal
  options): the host verifies, moves the folder aside, answers, and removes the files in the background without holding
  the checkout lock, finishing leftovers after a restart.
- **Space indication on the trash icon, not in the dialog.** In the maintainer's words: "not in trash dialog, have the
  trash icon have some kind of slightly animated indication it's being emptied until it's done and on hover we explain
  it."
- **The dialog closes at once.** "Instantly close the modal dialog (transition to just the trash icon animating as
  stated) so the user isn't waiting on it." Planner reading after the planning review, recorded as decided: this is the
  confirmed bulk delete; an entry's own Delete keeps the dialog open, since closing it after every single delete would
  get in the way of deleting several one by one.
- **An unremovable folder is moved anyway**; the background job logs a warning and retries after restarts. "Let's not
  worry about the UX for now other than nothing gets stuck/breaks." The maintainer asked for a Near term TODO entry to
  consider better visibility; the planning PR added it.
- **"In progress" means an attempt running now** (planner decision after the planning review), so an unremovable
  leftover does not animate the icon or keep the page polling forever.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** SPEC.md's managed checkouts and trash dialog (around 1432-1475) and its modal dialog
rule; SPEC_impl.md's "Archived-checkout trash" (around 4129-4215) and "Who owns an accepted action"; the operation
claim's contract in `crates/farhelm-ui/src/ops.rs` (module docs); SPEC_impl.md's looping-indicator rule, enforced by
`crates/farhelm-ui/js-tests/app-css-animations.test.js`; the descriptor budget in `checkout_trash.rs` (around 33-35,
"two reads and one delete"); the protocol's compatibility rules for an optional field (SPEC.md "Upgrade compatibility
and client scale"); `docs/desktop-web-triage.md`; root `AGENTS.md`, "Tests and validation", "Conventional Commits",
"Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and
`scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The reserved name is a dot-prefixed name inside `.farhelm-trash` (for example `.removing-<id>`), so it is on the same
  filesystem and the rename cannot cross devices; the listing skips records whose path has that form.
- The remover follows the spare discard path's shape: rename, then remove by verified handle.
- The re-read interval while a host says removing is a few seconds (on the order of 2-5 s).
- If any check before the rename needs data the removal used to provide (file counts in the completion log, say), log
  them from the background removal instead, keeping the per-step timing log.

## Outline

Three PRs:

1. `plan/instant-trash-delete/01-host-background-removal`: the rename under the lock, the record kept and hidden, the
   background remover, the resume at startup, the unremovable-folder handling, the listing's in-progress field with its
   proto and helm plumbing, and their tests. On its own this already makes today's dialog finish fast.
2. `plan/instant-trash-delete/02-trash-icon-emptying`: the dialog closing on a confirmed bulk delete, one request per
   host, the animated icon and its tooltip, the bounded re-read, dropping the page-wide claim, refusals kept for the
   next open, removing the per-checkout progress UI, and the browser tests.
3. `plan/instant-trash-delete/03-trash-specs`: SPEC.md, SPEC_impl.md, docs, the changelog fragment and the TODO.md
   removal, unless they fit better in the PRs above (keep each PR's spec text with its behavior where you can, and fold
   this PR away if nothing is left for it).

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy, the
supervisor's `checkout_trash` unit tests and the helm and proto trash tests through
`python3 scripts/record-test-run.py`, the CSS animation test (`crates/farhelm-ui/js-tests/app-css-animations.test.js`),
`python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares, and
`e2e/tests/trash-dialog.spec.ts` with any other spec that clicks the trash button, on both engines.

## Fallbacks

Examples of departures that need the scope reassessment: a new table or record file for leftovers, removing several
folders in parallel, a faster removal routine, carrying the in-progress state on the host or session listing, a notice
surface outside the trash dialog and the trash button's tooltip, or touching session Delete.

Decisions to log: the reserved name and how the listing hides it, what the remover does on each failure, the re-read
interval, the failure semantics where the old per-checkout queue and the new per-host batch differ, every test changed
and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-instant-trash-delete-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/instant-trash-delete/<nn>-<short-name>`,
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
