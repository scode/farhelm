# Light SQLite maintenance for the helm and supervisor databases

Written against main at b34ceee9f on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Tensorlake"). Verbatim:

> **Deep dive on SQLite compaction settings.** Review how Farhelm's databases handle compaction: WAL checkpointing
> (nothing sets `wal_autocheckpoint`, so SQLite's default of 1000 pages applies, and the supervisor's database module
> says there is no checkpoint policy), auto-vacuum, and page size. Decide whether the long-lived stores need explicit
> settings or a periodic maintenance step as they grow. Plan: `plans/queue/sqlite-compaction-settings.md`.

### Where it stands (from planning research at main b34ceee9f)

Farhelm keeps two SQLite databases: the supervisor's `supervisor.db` on every host that runs a supervisor (the local
machine, SSH hosts, sandboxes and containers), and the helm's `helm.db` beside the desktop app. Only `farhelm-helm` and
`farhelm-supervisor` link SQLite (rusqlite 0.40.2, bundled SQLite 3.53.2, built with `SQLITE_ENABLE_STAT4` and
`SQLITE_ENABLE_DBSTAT_VTAB`).

- Both stores open through one shared opener, `crates/farhelm-supervisor/src/db.rs`: `open_private` /
  `open_private_existing` (about lines 56-91: flags, mode 0600, the 5-second `BUSY_TIMEOUT`), then
  `configure_durability` (about lines 101-116), which only at an owning open tries the persistent WAL conversion
  (`try_enable_wal`, about lines 118-138, whose docs say "there is no retry task or checkpoint policy") and on every
  ordinary read-write connection selects `synchronous=NORMAL` once the file really is WAL. `with_full_sync` (about lines
  153-188) raises sync around trust-critical writes. `Db` (about lines 190-275) keeps one connection behind a mutex and
  runs every call on the blocking pool, so no in-process reader holds a long snapshot.
- Owning opens: the supervisor's `SessionStore::open(path, may_migrate)` (`crates/farhelm-supervisor/src/store.rs`,
  about lines 3366-3385), called from `crates/farhelm-supervisor/src/service/core.rs` (about line 6172) with
  `may_migrate` set from the state-directory claim; the helm's `HelmStore::open_inner` with a creation build
  (`crates/farhelm-helm/src/store.rs`, about lines 5712-5745), called by `HelmStore::open` from the helm's serve path
  (`crates/farhelm-helm/src/lib.rs`, about line 1409) after it claims serving ownership. Non-owning opens are the
  supervisor without the claim, `HelmStore::open_without_migration`, and the checkout-config CLI's separate opener
  `open_existing_inner` (`crates/farhelm-helm/src/store.rs`, about lines 5664-5709), which does not use the shared
  opener and keeps FULL (SPEC_impl.md says it is unchanged).
- Nothing sets `wal_autocheckpoint` (default 1000 pages, about 4 MiB at the default 4096-byte page),
  `journal_size_limit` (default -1: a `-wal` file is rewound, never truncated, while a connection is open),
  `auto_vacuum` (NONE) or `page_size`; production code runs no `wal_checkpoint`, `VACUUM`, `PRAGMA optimize` or
  `ANALYZE`.
- No table grows without bound: each growing table is capped or tied to something the user deletes. The main churn is
  the supervisor's `sessions` rows and the helm's `session_cache` (capped per host). One live supervisor measured in
  planning had a 154-page database and a WAL of about 1 MB.
- Orderly stop: the helm's serve path ends after its shutdown signal and stops its tunnels and manager
  (`crates/farhelm-helm/src/lib.rs`, about lines 1531-1547); the last writers there are the tunnels' shutdown and
  `manager.shutdown_and_wait()`. The desktop app's quit handler asks the embedded helm to stop under GPUI's 200 ms quit
  deadline without waiting (`HelmShutdown`, `crates/farhelm-desktop/src/lib.rs`, about lines 829-865). Only on Linux
  does anything then wait for the helm: GPUI's event loop returns and dropping the bootstrap waits for it (about line
  294). On macOS GPUI's quit ends in AppKit's `terminate:` (`vendor/gpui-pre-macos/src/platform.rs`, about line 594),
  and the process exits once the quit handlers return, so the helm's shutdown path normally does not get to finish
  there, and SQLite's own close-time checkpoint does not run either. The supervisor's serving lives in
  `Supervisor::serve` (`crates/farhelm-supervisor/src/service/core.rs`, about line 7998) with teardown in
  `service/teardown.rs`. Planning did not trace whether each store's connection is actually dropped (closed) on those
  paths.
- SQLite already checkpoints and deletes the `-wal` when the last connection to a database closes cleanly. It cannot do
  that when another connection is still open (a handoff successor, a CLI command), or when the process exits without
  dropping the connection.
- Spec text: SPEC_impl.md "Runtime state" for the supervisor (about lines 3945-3962) and the helm's state paragraph
  (about lines 5205-5222) describe WAL, sync and the sidecar files, and say nothing about checkpoints or size.
- Tests that read `sqlite_schema` / `sqlite_master` without filtering SQLite's own tables exist, for example in
  `crates/farhelm-supervisor/src/store.rs` (about lines 7240, 7287, 8769, 12657, 12710); some helm ones filter
  `name NOT LIKE 'sqlite_%'` (`crates/farhelm-helm/src/store.rs`, about line 17001).

### Acceptance criteria

1. Every read-write connection opened through the shared opener sets `journal_size_limit` to one fixed value in the 4-8
   MiB range, chosen and justified in its doc comment, so a `-wal` file's resting size is capped while the process runs.
2. An owning open, after its schema work, runs `PRAGMA optimize=0x10002` and logs one line with the database's label,
   `page_count`, `freelist_count` and the `-wal` file's size in bytes (0 when absent). A non-owning open (the supervisor
   without the claim, `open_without_migration`, the checkout-config CLI's opener) runs neither.
3. On orderly stop, a store opened with owning authority runs `PRAGMA wal_checkpoint(TRUNCATE)` as the last use of the
   store on that path, after every component that writes it has stopped, with a short, bounded wait: it never blocks
   shutdown for more than about a second, a busy result is logged and tolerated, and an error never fails the stop. A
   non-owning store does not. On macOS the helm's stop checkpoint is best effort and normally does not run, because the
   process exits after the quit handlers; that is accepted (see Decisions), and SPEC_impl.md and the report say so.
4. No `auto_vacuum`, `VACUUM`, `incremental_vacuum` or `page_size` change; no schema version bump; no protocol bump; the
   dead `web_token` / `device_sessions` tables stay.
5. The db.rs docs no longer say there is no checkpoint policy; they say what is set, where, and why. SPEC_impl.md's
   paragraph for each store says, in a sentence or two, what is now set and why.
6. Unit tests cover criteria 1-3 on temporary databases, including the stop checkpoint against a connection holding a
   read snapshot. That test asserts the observable outcome (the busy result logged and tolerated, the stop succeeding,
   the lowered busy timeout in effect for the call), never a wall-clock bound (`.agents/test-authoring.md`). Existing
   tests that list a database's tables still pass and still mean what they meant.
7. The TODO entry is removed.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Light hardening, not compaction:** `journal_size_limit` at read-write opens; `PRAGMA optimize=0x10002` at owning
  opens; a `wal_checkpoint(TRUNCATE)` on orderly stop that tolerates the busy timeout and never blocks shutdown badly; a
  log line at owning open with `page_count`, `freelist_count` and WAL bytes. All in the shared opener
  (`crates/farhelm-supervisor/src/db.rs`) used for `helm.db` and `supervisor.db`.
- **Authority:** maintenance runs only at owning opens, where WAL conversion already runs, never from the CLI's
  read-only or non-owning opens.
- **Out of scope:** `auto_vacuum` and `VACUUM`; a schema bump; a protocol bump; dropping the dead `web_token` /
  `device_sessions` tables.
- **Docs:** SPEC_impl.md's "Runtime state" sentences for each store say what is now set and why, replacing "there is no
  checkpoint policy" in the db.rs docs.
- **Where the stop checkpoint runs, and where it does not:** on the stores' existing orderly-stop paths only, after
  every writer has stopped. On macOS the process exits right after the quit handlers, so the helm's stop checkpoint
  normally does not run there; this is accepted, because `journal_size_limit` caps the resting `-wal` and the next open
  recovers it. Do not add plumbing to widen where the checkpoint runs: no work in the quit handler, no extension of the
  quit path or its deadline, no explicit closes, unwrapping of shared handles or new shutdown hooks. Report honestly
  what it covers.
- **Why it sits under Tensorlake:** the maintainer said there was no particular Tensorlake reason.
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- SPEC_impl.md's two store paragraphs quoted above, which this plan extends where acceptance criterion 5 says; the
  separate checkout-config CLI opener stays unchanged, as SPEC_impl.md says.
- `.agents/test-authoring.md` for every test added or changed; `docs/test-sleep-check.md` for the Rust test sleep check.
- Root `AGENTS.md`, "Conventional Commits", "Changelog fragments", "Tests and validation", and "Instructions by path".

**Planner proposals:**

- Put `journal_size_limit` in `open_private_with`, so every store open through the shared opener gets it whatever its
  authority (it is a per-connection setting, not maintenance), and leave the checkout-config CLI's own opener alone.
  Justify the value against the default autocheckpoint size: about 1000 frames of 4120 bytes is a little over 4 MiB, so
  a limit at or just above that keeps steady-state checkpoints from truncating and regrowing the file every cycle while
  still trimming the WAL after an unusually large transaction.
- Add one shared owning-open step in db.rs (beside `configure_durability`, taking the path so it can size the `-wal`)
  that each store calls after its schema work, under the same authority that gates migration: `PRAGMA optimize=0x10002`
  and the size line at info level.
- Add one `Db` method for the stop checkpoint that lowers the connection's busy timeout for the call (for example to a
  few hundred milliseconds, at most one second), runs `wal_checkpoint(TRUNCATE)` and logs its
  `(busy, log, checkpointed)` result. Call it from each store's orderly-stop path with owning authority, as the last use
  of the store there: on the helm after the tunnels' shutdown and `manager.shutdown_and_wait()`, on the supervisor after
  the rest of teardown. Nothing uses the connection afterwards, so restoring the busy timeout would be dead code;
  restore it only if you find a later user. First find out, and log, whether each store's connection is actually closed
  on orderly stop today: if it is and nothing else holds the file, SQLite's own close already empties the WAL, and the
  explicit step mainly covers the handoff overlap and a CLI connection open at the same moment. Say what you found in
  the report.
- Pitfall found in planning: `PRAGMA optimize` with the `0x10000` bit checks every table, and analyzing a table with
  unanalyzed indexes creates SQLite's `sqlite_stat1` (and, with STAT4, `sqlite_stat4`) tables. Tests that list every
  table or dump the whole schema after an owning open will see them. Those tests describe Farhelm's schema, so filtering
  SQLite's internal `sqlite_%` names in their queries keeps their meaning; adjust each such test and say why in its
  comment rather than weakening its assertion. Also check that no open path or schema check treats an unknown table as a
  sign of a newer schema, since an older build reopening the file after a downgrade will meet these tables.
- The stop checkpoint never goes into the desktop app's quit handler or its 200 ms deadline: it sits on the helm's own
  shutdown path, which runs to the end on Linux and normally not on macOS (see Decisions).

## Outline

1. `plan/sqlite-compaction-settings/01-sqlite-light-maintenance`: the opener's `journal_size_limit`, the owning-open
   step, the stop checkpoint and its callers, the db.rs docs, SPEC_impl.md's two paragraphs, the tests, and the TODO
   entry's removal. `chore:` with no changelog fragment: nothing a user can notice changes (a `-wal` file that rests a
   few MiB smaller at most, planner statistics on tables of hundreds of rows, a log line), and root `AGENTS.md`,
   "Conventional Commits", has the type follow the user-visible effect. If your measurements show an effect a user could
   notice, use `perf:` with a `kind: changed` fragment instead and log why.

## Fallbacks

- A store or platform whose stop ends by process exit (the helm on macOS, and any other you find) gets no stop
  checkpoint there: log it and say so in the report, without adding plumbing to make room for one (see Decisions). Where
  an existing orderly-stop path does run to its end, put the call at the end of it, after the last writer.
- If `PRAGMA optimize` turns out to make an owning open noticeably slower on a realistically sized database (measure on
  a temporary database seeded to the caps), keep it and log the measurement unless it is over a second, in which case
  drop it from this PR and report it as a follow-up.
- A test that breaks for any reason other than SQLite's own `sqlite_%` tables appearing is a red flag: investigate
  before changing it.
- Anything that would need a schema or protocol bump is out of scope: record it as a follow-up.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-sqlite-compaction-settings-log.md` in the parent directory of the checkout you run in, derived as that
section says, and the watchdog's status file is `farhelm-plan-sqlite-compaction-settings-watchdog.json` beside it.
Resolve both to absolute paths before you start; this file does not create either. Scratch files go in the agent scratch
directory that `AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Choose the narrowest checks that exercise what each PR changes. Apply `.agents/test-authoring.md` to every
test you add or change, and run `python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it when
Rust tests change. Never take a state-changing action against the live Farhelm install on this machine (root
`AGENTS.md`, "The live install is off-limits"); in particular, do not open or measure the installed app's databases.

For this plan that means at least the db.rs tests and both stores' test modules (the supervisor's and the helm's, which
hold the schema-listing tests), plus the tests of whatever orderly-stop code you touch, and clippy on the changed
crates.

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
  `plan/sqlite-compaction-settings/01-sqlite-light-maintenance`.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Deep dive on SQLite compaction settings"), as
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
