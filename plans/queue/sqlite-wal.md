# Supervisor and helm: run SQLite in WAL mode with synchronous NORMAL

Written against main at 5831410e on 2026-10-30. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Switch SQLite to WAL with synchronous NORMAL.** The supervisor's database uses SQLite's defaults (rollback journal,
> synchronous FULL), so every commit waits for several disk flushes; on a loaded host that was 0.2–0.6 s per small
> write, with the one shared connection held throughout. WAL with NORMAL keeps crash consistency and gives up only the
> last commits before an OS crash or power loss, which is acceptable here. The switch converts the existing database in
> place on open; the pragma can return the old mode instead of failing while another connection holds the database (the
> upgrade handoff), and synchronous must be set on every open. Consider the helm's database too.

The maintainer decided, on the planning page, that the helm's database switches too.

How it works today, from a read of main at 5831410e (line numbers approximate):

- Both stores open through `crates/farhelm-supervisor/src/db.rs` (`open_private`, `open_private_existing`: explicit
  flags, mode 0600, a 5 s busy timeout) and keep one connection behind a mutex (`Db`). No code sets `journal_mode` or
  `synchronous`, and neither spec mentions them.
- Supervisor: `SessionStore::open(path, may_migrate)` in `crates/farhelm-supervisor/src/store.rs` (around 2837-2842),
  called from `Supervisor::new_claiming` in `crates/farhelm-supervisor/src/service/core.rs` (around 5560-5701) with
  `may_migrate` true only when this process holds `supervisor.lock`. A claimless supervisor (the brief handoff overlap)
  still opens the file but is read-only by design and never migrates.
- Helm, `crates/farhelm-helm/src/store.rs`: `open_inner` (around 4494-4545) behind `HelmStore::open` (the serving helm,
  `crates/farhelm-helm/src/lib.rs` around 2251, after it claims serving ownership; also `token show` when it gets
  ownership), `open_existing` (offline `token rotate`, holding ownership) and `open_without_migration` (`token show`
  beside a serving helm). Separately, `open_existing_inner` (around 4447-4492) opens with its own flags, read-only for
  `checkout-config show`, for the `farhelm helm checkout-config` CLI (`crates/farhelm-helm/src/checkout_config.rs`
  around 1586-1588), which runs beside a serving helm.
- `checkout_config_snapshot_reads_one_consistent_view` (`checkout_config.rs` around 2775) asserts `journal_mode` is
  `delete`; its oracle relies on readers blocking commits. It opens both connections with `HelmStore::open`.
- The desktop app's recovery text (`crates/farhelm-ui/src/desktop.rs` around 1147-1153, test around 1706-1716) tells the
  user to "restore both files from the same backup" (`supervisor.db` and `helm.db`).
- SPEC.md, "Power-loss durability of terminal file downloads and checkouts" (around 3407-3415), keeps the databases
  under their existing contracts and keeps the ban on database corruption. SPEC.md, "Upgrade compatibility and client
  scale" (around 4054-4090), requires every stable release since v0.23.0 to update normally onto its existing state.

SQLite behavior the design rests on, checked by an independent planning review with probe scripts against the system
SQLite (confirm the busy cases with unit tests against the bundled library, `rusqlite` 0.40 with `bundled`):

- `PRAGMA journal_mode=WAL` is a persistent property of the file. It succeeds while another connection is merely open
  and idle, and that connection follows along. While another connection is inside a read transaction it waits out the
  busy timeout and then fails with `SQLITE_BUSY`; while another holds the write lock it fails at once. It does not
  quietly return the old mode in those cases, despite the entry's wording.
- `synchronous=NORMAL` is safe from corruption only in WAL mode. In rollback-journal mode SQLite documents a small
  chance of corruption on power loss with NORMAL, which SPEC.md forbids. So NORMAL may only be set on a connection whose
  file is actually in WAL mode.
- Older releases bundle the same SQLite and open a WAL database transparently, so going back to an older build still
  opens the file. The brick test's `?mode=ro` reads and read-only opens work under WAL.

### Acceptance criteria

- Opening a database with the right to migrate (the serving supervisor, the serving helm, an offline `token rotate`,
  `token show` holding ownership) converts it to WAL in place, before `apply_schema` and outside any transaction. A
  conversion that errors or does not report `wal` is logged as a warning and the open carries on in the file's current
  mode; it must never make the supervisor or helm fail to start. The next owning open simply tries again; no other retry
  mechanism. State in a comment that a busy conversion can add up to the busy timeout to startup.
- Every read-write open, migrating or not, then reads the file's actual journal mode and sets `synchronous=NORMAL` only
  if it is `wal`; otherwise it leaves the default FULL.
- Openers without the right to migrate never try to convert. `open_existing_inner` (the `checkout-config` CLI) stays as
  it is: the file's WAL mode applies to its connection by itself, and its rare writes at FULL are harmless.
- Each store reaches the shared setup through `db.rs`, so the supervisor and the helm cannot drift apart.
- `checkout_config_snapshot_reads_one_consistent_view` is rewritten for WAL: the racing commit lands, and the reader in
  its transaction still sees the old value and the old revision. Unit tests cover: a fresh and an existing rollback-mode
  database come out in WAL with NORMAL after an owning open; a non-owning open of a rollback-mode file leaves it in
  rollback mode at FULL; a conversion that hits `SQLITE_BUSY` (another connection in a read transaction, a short busy
  timeout if the test needs one) still opens, in the old mode at FULL, and logs the warning.
- The desktop recovery text names the `-wal` (and `-shm`) files beside each database, since committed data can sit in
  the `-wal` file after an unclean stop; its test is updated with it.
- SPEC_impl.md's two "State in SQLite" passages (around 2954 and 3933) state the journal mode, the sync level, who
  converts, the fallback, and the durability tradeoff: an OS crash or power loss can lose the last commits but cannot
  corrupt the database. One line notes that older builds still open a WAL database.
- The report lists any places the executor noticed where a lost last commit could leave a database disagreeing with a
  filesystem change already made or with a success already reported (for example a record updated just before a folder
  moves). Listing only; no change to those writes.
- A changelog fragment per root `AGENTS.md`, "Changelog fragments", and the last code PR removes the TODO.md entry.

## Decisions already made

Settled with the maintainer on 2026-10-30, on the planning page:

- **Both databases switch:** the supervisor's on every host and the helm's.
- **Losing the last commits on an OS crash or power loss is accepted for every write.** No write keeps FULL sync. The
  executor lists in its report any spots it notices where such a loss could leave a database disagreeing with a
  filesystem change or a reported success; nothing blocks on them.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry quoted above and the decisions above. Where the entry says the pragma "can
return the old mode instead of failing", the review found it fails with `SQLITE_BUSY` instead; the entry's intent, that
a busy database must not stop the process, is what binds.

**Binding repository constraints:** SPEC.md, "Power-loss durability of terminal file downloads and checkouts" (no
database corruption) and "Upgrade compatibility and client scale"; SPEC.md, "Healthy local filesystems"; SPEC_impl.md's
store sections (around 2954 and 3933); the rule that only the owner of the state directory migrates or writes durably
(`db.rs` module docs, the `apply_schema` docs in both stores, `crates/farhelm-helm/src/token_control.rs` around
117-219); root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments", "TODO.md and
FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Two small helpers in `db.rs`: one for the conversion, called only by owning opens, and one for the sync level, called
  by every read-write open after it. A parameter on the open functions is fine too, if it reads better.
- No change to transaction styles. An independent review found no new hazard: a deferred read-then-write that meets a
  concurrent writer fails at once in rollback mode too, the claimless supervisor never writes, and WAL only narrows that
  race. Converting the supervisor's deferred transactions to IMMEDIATE was considered and dropped.
- No checkpoint-on-shutdown or other WAL maintenance: SQLite's automatic checkpoints are enough at this size.

## Outline

One PR:

1. `plan/sqlite-wal/01-wal-normal`: the `db.rs` helpers and their calls in both stores, the tests, the rewritten
   snapshot test, the desktop recovery text and its test, the SPEC_impl.md passages, the changelog fragment and the
   TODO.md removal.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-supervisor`, `farhelm-helm` and `farhelm-ui`, `python -B scripts/check-test-sleeps.py` with the interpreter
`docs/test-sleep-check.md` prepares, and, through `python3 scripts/record-test-run.py`, the store and `db.rs` unit tests
in `farhelm-supervisor` and `farhelm-helm` (including `checkout_config`, `token_control` and the concurrent-open tests),
the desktop recovery-text test, and `crates/farhelm/tests/checkout_config_cli.rs`. Consider one supervisor handoff or
upgrade test from the inventory, since the handoff overlap is where the conversion can meet a busy file.

## Fallbacks

Examples of departures that need the scope reassessment: changing transaction styles, a retry or background job for the
conversion, checkpoint management, keeping FULL for particular writes, changing `open_existing_inner`, or touching the
state-directory locking.

Decisions to log: where the helpers live and how each open path calls them, how the busy conversion was tested against
the bundled SQLite, every test changed and why, every place listed for the report under the durability decision, and
every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-sqlite-wal-log.md` in the parent directory of the checkout you run in, derived as that section says.
Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/sqlite-wal/<nn>-<short-name>`, as the
  outline names them.
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
Wording in this file that turns out inaccurate (a line number, a file name, a SQLite behavior the bundled library does
not show) is corrected to the truth.

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
