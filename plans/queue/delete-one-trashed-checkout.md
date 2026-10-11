# Delete one checkout from the trash

Written against main at 3d97d46b on 2026-10-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

The TODO.md entry (Near term), "Delete one checkout from the trash": the trash dialog (archived managed checkouts,
listed per host) can only delete everything on one host or everything on every host. Add a delete button on each
checkout so a single one can go without taking the rest with it.

Acceptance criteria:

- Every checkout entry in the trash dialog has its own delete button. Clicking it deletes that checkout permanently,
  right away, with no confirmation step. Nothing else on that host or any other host is touched.
- The button is unavailable whenever the host's own delete is: its listing is not settled, a host-list refresh failed,
  the host's listing exceeds the cleanup limit (one ID is not an exception), another trash deletion (single, per host or
  global) is still running, or another operation holds the shared operation guard. An unreachable host shows no entries,
  so it shows no buttons either.
- While the single delete runs, the host shows "deleting…" and the dialog header announces it, as for the other deletes
  (SPEC.md requires the host-level line), and the dialog cannot be closed. When the reply comes the entry goes away or
  the failure is shown on that host, exactly as a per-host delete's result is shown today, the listing is refetched, and
  focus goes to Close, as after every other delete.
- The per-host and global deletes, their confirmations, progress and focus behaviour are unchanged.
- SPEC.md "Managed checkouts" says, in the same PR, that each checkout can also be deleted on its own, at once, with no
  confirmation and so no warning about uncommitted or unpushed work; and the paragraph that begins "An explicitly
  confirmed trash delete is the permanent-cleanup exception" is reworded so that it covers the single delete too, with
  the same ownership checks.
- Tests: a UI unit test where the selection logic gains anything testable, and a browser test (below).
- One draft PR, passing its review gate, with its changelog fragment, and removing the TODO.md entry.
- No PR is marked ready, and nothing is merged.

## Requirement sources

**The user's request (2026-10-11):** plan this TODO entry, through the todo planning flow.

**The user's decisions (2026-10-11):**

- A single checkout's delete button deletes immediately, with no confirmation. (Offered and not chosen: the same
  in-place confirmation the per-host and global deletes use, on that entry.) This is deliberately unlike the other two
  deletes; do not add a confirmation, an undo, or a "are you sure" on your own.
- Review gate: a single fresh-context gpt-6.1-sol reviewer at high effort, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires.

**Planner decisions, stated to the user without objection (2026-10-11):** the per-checkout button is on checkout entries
only. A broken trash record that shows only as an issue notice (no checkout row) gets no button of its own and is still
forgotten by the per-host and global deletes, as today.

**Binding repository constraints:**

- SPEC.md "Managed checkouts", the trash paragraphs (trash button, dialog, per-host and global delete, deletion
  progress, close blocked while deleting). SPEC_impl.md "Archived-checkout trash": IDs select records, never
  browser-supplied paths; confirmations freeze deduplicated IDs and observed host incarnations; before dispatch the UI
  rechecks connection identity and takes the shared operation guard; successful removals fold into the listing and every
  completion refetches; a lost reply warns that some contents may already be deleted. A single delete keeps all of these
  except the confirmation step: it freezes its one ID and the host's incarnation at click time.
- SPEC.md "Upgrade compatibility and client scale". No protocol change is expected (see Outline); if you find one is
  needed, that is a scope reassessment.
- Root `AGENTS.md`: Conventional Commits; Finishing work (browser tests through the recorder on Chromium and WebKit, and
  `scripts/check-test-sleeps.py` for any test delay); Releases and the changelog; Agent scratch space; Sharing the
  machine with other agents.
- `.agents/test-authoring.md` for every test change. `plans/AGENTS.md`, Executing one plan.

**Planner proposals** are the mechanism and test shapes in the outline below, checked against main and corrected by a
fresh-context planning review.

## Outline

Line numbers drift; find the code by name.

Here's what exists today:

- `crates/farhelm-ui/src/list/trash.rs`, component `TrashControl`. Each host is a `section.checkout-trash-host`; its
  `.trash-host-delete` button builds a `Target` with `target()` and sets a `Confirmation { host: Some(id), targets }`;
  the header's `.trash-delete-all` sets `Confirmation { host: None, .. }`. `ConfirmBand`'s submit runs the `submit`
  closure, which revalidates host incarnations, claims the shared operation guard (`gate.claim_guard()`), records a
  `DeletionRun` of pending hosts, calls `crate::api::empty_checkout_trash` once per target, folds `removed` IDs out of
  the shown listing, puts issues in `notices`, and on the last reply releases the guard, refetches, and focuses Close.
  Entries (`div.checkout-trash-entry`, keyed by `checkout.id`) have no button today. The row-archive cue (`capture_row`,
  `finish_row_cue`) is unrelated.
- The helm and supervisor path already deletes any subset of IDs: `/api/checkout-trash/delete` with
  `DeleteCheckoutTrashReq { host, expected_incarnation, ids }`, proto `DeleteCheckoutTrash { req_id, ids }`, supervisor
  `checkout_trash::delete`, which checks each ID's ownership evidence and that no session uses it. One ID is an ordinary
  request. So this is a UI-only change; no helm, supervisor or protocol change.

Here's the work:

1. **Factor the dispatch.** Split `submit` so that the part after the confirmation (revalidate, claim the guard, record
   the run, send, fold, release, refetch) takes a list of `Target`s, and is called both by `ConfirmBand`'s submit and by
   the new button. Keep the per-host and global behaviour byte-for-byte the same.
2. **The button.** In each `checkout-trash-entry`, a small delete control (class e.g. `trash-entry-delete`, an
   accessible name that includes the checkout's name, the dialog's existing danger styling). On click it builds a one-ID
   `Target` for that host (the checkout's ID, the host's current incarnation, that checkout's size) and dispatches it.
   Disabled as `busy || gate.busy() || host_target.is_none()`: the host's own delete's conditions, plus the shared
   operation guard, because the per-host button only opens a confirmation whose submit checks the guard, while this one
   dispatches directly and would otherwise do nothing when the guard is held. The shared dispatch already clears any
   open confirmation.
3. **Progress and focus.** A one-ID `Target` through the shared dispatch gets
   `DeletionRun { pending: {host}, total: 1 }`, which already gives the header announcement, the host's "deleting…"
   line, Close and Escape blocked, every delete control disabled, and focus on Close at the end. Keep all of that.
   Showing "deleting…" on the entry itself as well is optional (it needs a small set of in-flight IDs, since
   `DeletionRun` knows hosts); log the DECISION. Focus goes to Close as after every other delete: when the reply lands,
   the host's other entry buttons are disabled by the refetch and an emptied host's section disappears, so there is
   nothing better to focus.
4. **SPEC.md**: one sentence in the trash paragraph of "Managed checkouts" (single delete, immediate, no confirmation,
   no warning), and the "An explicitly confirmed trash delete is the permanent-cleanup exception" paragraph reworded to
   cover it. SPEC_impl.md "Archived-checkout trash": one sentence that a single-entry delete freezes its one ID and the
   host's incarnation at click time and goes through the same dispatch.
5. **Tests.**
   - A unit test in `trash.rs` for the one-ID target builder, if it becomes a function of its own: exactly that entry's
     ID and the host's incarnation; never another entry's ID or an issue's ID. The size fields are only read by the
     confirmation, so do not assert them as if they mattered.
   - A Playwright test in `e2e/tests/trash-dialog.spec.ts`, which already serves route-mocked listings
     (`fulfillAsHelm`). Its `stubArchives` fixture records deletions per host, so give it a per-ID removed set first;
     otherwise the refetch either brings the deleted entry back or empties the whole host. Then: deleting one entry
     sends exactly one delete request with exactly that ID, the host shows deleting, the entry disappears, and the other
     entries and hosts stay. The buttons are disabled while a deletion runs and while a host-list refresh has failed,
     and an unreachable host shows none. Add a failure case if the existing failure tests make it cheap.
   - If `e2e/tests/github-checkouts.spec.ts`'s real-archive tests can cover a single delete with little extra setup, add
     one case there too, since it is the only coverage against a real supervisor; log the DECISION either way.
6. **Bookkeeping.** Changelog fragment, `kind: added`. Remove the TODO.md entry.

### Validation

Follow root `AGENTS.md` "Finishing work", choosing the smallest set that covers the change:

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`;
- the UI crate's unit tests through the recorder (narrowly, `-p farhelm-ui`);
- the changed Playwright specs on Chromium and WebKit through the recorder, after the builds the browser suite needs;
- `python -B scripts/check-test-sleeps.py` per `docs/test-sleep-check.md` if any test delay is added;
- `dprint check` on changed Markdown, and `python3 releasing/check-changelog.py format`.

Do not run the full battery without a specific risk that calls for it.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-delete-one-trashed-checkout-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

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
  `plan/delete-one-trashed-checkout/<nn>-<short-name>` (for example `plan/delete-one-trashed-checkout/01-entry-delete`).
- One PR. Keep it focused and avoid churn: code added in one PR and deleted in a later one means the stack should have
  been shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and
  that applies to PRs an earlier run built too.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect (`feat`). A `feat`
  PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the
  changelog). Validate with `python3 releasing/check-changelog.py format`.
- The PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing one cannot reach it
natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name:

- the repository root (the executing checkout);
- the bookmark or commit range;
- a findings file in the scratch directory;
- the acceptance criteria: this file's goal, the user's decisions, and the outline.

The PR changes tests, so include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.
Ask the reviewer to check:

- that a single delete can only ever send the one ID it was clicked for, with the incarnation seen at click time;
- that the per-host and global paths behave exactly as before;
- that no click can start a second deletion while one runs, and that the guard is always released;
- that the absence of a confirmation is the user's decision and not a finding.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

- before implementing a substantial departure from the outline above, for example a helm, supervisor or protocol change,
  a new endpoint, an undo or restore facility, or a selection model with checkboxes (examples, not a blacklist);
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
alternatives considered. In particular:

- how the dispatch was factored;
- whether the entry itself shows "deleting…";
- the button's placement, label and accessible name;
- the SPEC.md and SPEC_impl.md wording;
- whether a real-archive browser case was added;
- which browser specs ran;
- every review finding you decided not to follow.

The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. Examples: adding a confirmation or undo to the single
delete, adding buttons to issue-only records, multi-select, or any change to what the supervisor will delete.

If no fallback covers it, record the concrete tradeoff and continue with independent work. Block per `plans/AGENTS.md`
(Executing one plan, step 10) only for what cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
