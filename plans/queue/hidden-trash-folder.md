# Supervisor: move the managed-checkout trash into a hidden folder

Written against main at 463cac15 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Move the trash into a hidden folder.** Deleting a managed checkout's last session moves the checkout into
> `farhelm-archived-working-copies` inside the checkout folder (`ARCHIVE_DIR_NAME` in
> `crates/farhelm-supervisor/src/working_copies.rs`; SPEC.md, Managed checkouts, and the ownership statement that names
> it), a visible folder beside the user's own checkouts. Move it into a hidden folder named in the same style as the
> spare checkouts' `.farhelm-spares` (planned in `plans/queue/spare-launch-checkout.md`), for example `.farhelm-trash`;
> settle the exact name with that plan's. Needs: migrating existing trash folders on every host (archived checkouts are
> recorded by path in the registry's `archive_destination`, so the move updates those records, and a same-filesystem
> rename keeps the identity checks intact); the reserved-name handling in the occupancy scan
> (`occupied_related_names_from_entries`) and the nesting and overlap guards that compare against the archive path;
> repository discovery's skip (`repository_discovery.rs`, which skips the archive folder by name); the path-length
> margin (`ARCHIVE_PATH_GROWTH`, 82 bytes, quoted in SPEC.md, changes with the name); and the text in SPEC.md,
> `docs/github-checkouts.md` and the docs page `website/src/content/docs/docs/using/stop-restart-resume.mdx`. Asked for
> by the maintainer on 2026-10-20 while planning the spare checkouts.

The spare checkouts work has landed since that entry was written (`SPARE_DIR_NAME = ".farhelm-spares"` in
`working_copies.rs`), and the entry's claim that `archive_destination` holds a path is wrong: the registry stores a bare
entry name inside the root's archive folder (see "What the code does today"). Renaming the folder is enough; no registry
record or schema changes.

### Acceptance criteria

- Deleting a managed checkout's last session moves the checkout into `<checkout root>/.farhelm-trash/` instead of
  `<checkout root>/farhelm-archived-working-copies/`, with the same naming (original basename, UTC timestamp, collision
  suffix), the same no-overwrite move and the same identity guards.
- Existing trash moves over on its own, per checkout root, the first time the supervisor touches that root's trash in
  any way after the upgrade: archiving a checkout, recovering an interrupted archive move, listing the trash (the count
  beside New and the dialog), or deleting from it. The move is a no-replace rename of the whole legacy folder to the new
  name, followed by an fsync of the root. A root that was unreachable at startup and comes back later is migrated before
  anything reads its trash, so the trash list's existing prune of missing archives never forgets checkouts that are
  merely still in the legacy folder.
- An archive move left half done before the upgrade (a row in `archive_pending` whose rename had already completed into
  the legacy folder) finishes correctly after the upgrade, ending under the new folder.
- When the legacy folder does not exist, nothing happens. When both folders exist (only reachable through out-of-band
  changes, which SPEC.md's ownership statement says Farhelm adds no complexity to handle), the legacy folder is left
  alone and the new one is used. A migration rename that fails for any other reason surfaces as the existing error paths
  do (an issue in the trash list; the "not archived" path with its notice on Delete) and never falls through to creating
  an empty new folder.
- The migration is skipped on a root where a non-retired managed checkout is, contains, or sits inside the legacy folder
  (a registry from before v0.18.0, which first reserved the name, can hold one). That user checkout is never renamed.
- Trash delete keeps SPEC.md's promise to refuse an archive containing a retained session's working folder, also for a
  session whose recorded folder is still spelled with the legacy name because the trash moved under it.
- The legacy name stays reserved: allocation never offers it as a checkout name and repository discovery never treats it
  as a repository. The name-allocation callback in `service/core.rs` (around 4980), which compares against the archive
  constant alone, uses `reserved_checkout_name` instead, so the spare container and both trash names are covered.
- `ARCHIVE_PATH_GROWTH` follows the new name (1 + 14 + 17 + 1 + 32 = 65 bytes with `.farhelm-trash`), its const assert
  and SPEC.md's quoted figure with it.
- SPEC.md (Managed checkouts, around 1137; the path figure, around 1031; the ownership statement, around 3638),
  `docs/github-checkouts.md` (around 120) and `website/src/content/docs/docs/using/stop-restart-resume.mdx` (around 87)
  name the new folder, and SPEC.md says that existing trash moves to it automatically.
- Tests, fixtures and doc comments that name the folder use the new name, and the migration has tests (see Outline).
- A changelog fragment, kind `changed`, says the trash folder is now hidden and that existing trash moves on its own,
  with the caveat below about sessions started inside a trashed checkout. Per root `AGENTS.md`, "Changelog fragments",
  it does not mention downgrades.
- The TODO.md entry above is removed in the same PR.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **The folder is `.farhelm-trash`.** It matches `.farhelm-spares`, which sits beside it in the checkout root.
- **Migration renames the legacy folder before any use of that root's trash**, rather than only at supervisor startup
  (which would let a root unreachable at startup have its trash forgotten later) or not migrating at all (which would
  mean checking both names indefinitely).
- **Losing the downgrade path is accepted.** After the migration, an older release on that host finds no trash, and its
  trash list drops every trashed checkout; the folders and their work stay on disk in `.farhelm-trash` but Farhelm stops
  managing them. SPEC.md, "Upgrade compatibility and client scale", asks that such a loss be raised during design; it
  was, and the maintainer accepted it.
- **Accepted caveat:** a session started inside a trashed checkout (to copy work out, say) loses its folder when the
  trash moves, so restarting it fails afterwards. Permanent trash delete still recognizes it and refuses that checkout.
  This was stated on the planning page; no further mechanism (such as skipping the migration while such a session
  exists) is wanted.
- **One PR**, so SPEC.md never names a different folder from the code between merges.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## What the code does today

From a read of main at 463cac15 (line numbers approximate), confirmed by an independent fresh-context review during
planning:

- `crates/farhelm-supervisor/src/working_copies.rs`: `ARCHIVE_DIR_NAME` (around 76), `SPARE_DIR_NAME` and
  `RESERVED_CHECKOUT_NAMES` with `reserved_checkout_name` (around 80-91, case-insensitive, shared by allocation and
  discovery), `MAX_ADMITTED_CHECKOUT_PATH` and `ARCHIVE_PATH_GROWTH` with its `== 82` assert (around 93-110),
  `refuse_overlapping_archive` and `refuse_archive_destination_checkout` (around 1044-1093, path computations only),
  `archive_move_with_effects` (calls `ensure_archive_root` around 1710), `ensure_archive_root` (around 1757; creates the
  folder when missing), and `reconcile_archive_with_effects`, which calls `ensure_archive_root` (around 2065) **before**
  it looks for the journaled destination.
- `crates/farhelm-supervisor/src/checkout_trash.rs`: `archive_name` requires a single-component bare name;
  `open_archive` (around 112) opens `<root>/ARCHIVE_DIR_NAME/<name>` from the verified root descriptor and returns
  `Ok(None)` on ENOENT; `list` (around 318) and `delete_with` (around 368) call `forget_retired` on `Ok(None)`. That
  prune is why the migration must run before either reads. `delete_with`'s retained-session guard (around 420-427)
  compares recorded session folders against `archive.path` and its canonical form. `diagnostic_path` computes display
  text only. `list` runs without directory admission (`service/core.rs` around 5063-5075).
- `crates/farhelm-supervisor/src/service/teardown.rs`: `journaled_destination_path` (around 112) builds notice text.
- `crates/farhelm-supervisor/src/service/core.rs`: planned checkout path refused inside the archive folder (around
  2361), the name-allocation callback (around 4980), test literals (around 26468, 33149, 37284).
- `crates/farhelm-supervisor/src/repository_discovery.rs`: the production skip goes through `reserved_checkout_name`
  (around 156); around 481 is a test loop over the reserved names.
- `crates/farhelm-supervisor/src/spare_checkouts.rs`: spares keep their own trash names inside `.farhelm-spares` and are
  unaffected. `rename_at_noreplace` (in `working_copies.rs`) maps EEXIST/ENOTEMPTY to `AlreadyExists`.
- Other references: `crates/farhelm-helm/src/sessions_tests.rs` (around 10867, a fake path), browser specs
  `e2e/tests/github-checkouts.spec.ts` (reads the real folder), `trash-dialog.spec.ts`, `modal-layout.spec.ts`,
  `popover-width-movement.spec.ts` (fake path strings), `e2e/docs-shots/stage-docs.ts` (around 223, a staged path), and
  `e2e/readme-hero/scenario.ts` (around 99, a comment). Doc comments naming the folder: `working_copies.rs` around 97
  and 1624, `core.rs` around 2331, `teardown.rs` around 2748.
- Frozen material that also names the folder and must not be edited: `CHANGELOG.md`, `TRIAGE_OUTCOMES.md`, `lore/` (root
  `AGENTS.md`, "Frozen material").

## Requirement sources

**The user's request:** the TODO.md entry above and the decisions above.

**Binding repository constraints:** SPEC.md, Managed checkouts (around 1020-1175, the archive move, the trash dialog and
trash delete's retained-session refusal), the ownership statement (around 3637-3651), and "Upgrade compatibility and
client scale" (around 3700-3720: the upgrade path from v0.23.0 on must keep working); SPEC_impl.md's "Archived-checkout
trash"; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"
and "Frozen material"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`;
`.agents/docs-media.md` and `docs/docs-shots/SPEC.md` before touching `e2e/docs-shots/` or `e2e/readme-hero/`;
`website/AGENTS.md` before touching `website/`.

**Planner proposals, shaped by the planning review** (change them if the code says otherwise, logged as a DECISION):

- Rename the constant to the new name and keep the old one as a legacy constant in `RESERVED_CHECKOUT_NAMES`.
- One helper, for example `migrate_legacy_archive`, doing the no-replace rename from the opened root plus the root
  fsync: ENOENT and `AlreadyExists` are no-ops, anything else propagates. It needs a registry connection for the
  legacy-overlap skip, which reuses `refuse_archive_destination_checkout`'s check pointed at the legacy path (a small
  parameterization). It is called in exactly two places: at the top of `ensure_archive_root`, before its
  `symlink_metadata`/`create_dir` (this covers both archiving and recovery), and in `open_archive` after the root is
  verified (this covers list and delete; `open_archive` needs the connection passed in).
- The legacy-spelling check in `delete_with`'s retained-session guard is a `starts_with` on `<root>/<legacy>/<name>`,
  beside the existing comparison.
- Running the migration from the trash list outside directory admission is fine: concurrent callers race on the same
  no-replace rename, one wins and the rest see ENOENT. Say so in the helper's doc comment.
- Leave the published docs screenshots alone; the staged path in `e2e/docs-shots/stage-docs.ts` changes, and the
  screenshots pick it up at the next refresh. Note that in the report.

## Outline

One PR, `plan/hidden-trash-folder/01-hidden-trash`, type `feat` or `fix` per the user-visible effect (likely `feat`),
holding: the constant rename and legacy reservation, the `core.rs` allocation callback fix, the migration helper at its
two call sites, the retained-session guard's legacy spelling, the path margin, SPEC.md, the two docs, test fixtures and
doc comments, the new tests, the changelog fragment and the TODO.md removal.

Tests to add (Rust, in the existing test modules of the files involved):

- legacy folder only: migrated on first archive, on first list and on first delete, with trashed checkouts still listed
  afterwards;
- neither folder: nothing created by a list; the first archive creates `.farhelm-trash`;
- both folders: the legacy folder is left untouched and archives go to the new one;
- a pending archive whose rename completed into the legacy folder, recovered after the upgrade, completes under the new
  folder (the ordering case inside `ensure_archive_root`);
- a migration rename that fails with an error other than ENOENT/EEXIST does not create the new folder;
- a non-retired checkout at the legacy path: no migration, the checkout is untouched;
- trash delete refuses an archive whose retained session's recorded folder uses the legacy spelling;
- the legacy name stays reserved in allocation (including the `core.rs` callback) and discovery.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-supervisor` (and `farhelm-helm` if touched), the `farhelm-supervisor` tests for `working_copies`,
`checkout_trash`, `service::teardown`, `service::core` archive and allocation tests and `repository_discovery` through
`python3 scripts/record-test-run.py` with the pinned nextest setup, `python -B scripts/check-test-sleeps.py` with the
interpreter `docs/test-sleep-check.md` prepares, `python3 releasing/check-changelog.py format`, and the browser specs
whose path strings change (`e2e/tests/github-checkouts.spec.ts` above all, since it reads the real folder) on both
engines.

## Fallbacks

Examples of departures that need the scope reassessment: a startup sweep over all roots, a registry or schema change, a
fallback lookup in the legacy folder on reads, merging two existing folders, skipping the migration while sessions sit
inside the trash, or any downgrade-compatibility mechanism.

Decisions to log: where exactly the helper hooks in if the call sites differ from the above, how the connection reaches
`open_archive`, every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-hidden-trash-folder-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/hidden-trash-folder/<nn>-<short-name>`,
  as the outline names them.
- Keep each PR bite-sized; this plan's decision is one PR. Avoid churn: code added in one PR and deleted in a later one
  means the stack should have been shaped differently. Within this run, a PR that needs correcting is restructured
  rather than corrected on top, and that applies to PRs an earlier run built too.
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
