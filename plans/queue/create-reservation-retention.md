# Retry keys live as long as the session they created

Written against main at 2897c914 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan has no dependency on another plan. `supervisor-request-modes.md` landed on 2026-10-21, after this file's code
references were taken: it rewrote the supervisor's create and reconciliation paths
(`crates/farhelm-supervisor/src/service/core.rs` and `service/handlers.rs`) that this plan changes, so re-find every
cited location in those files on current main before relying on it.

## The goal

When a client attaches a retry key (an intent key) to a create, the supervisor records it so that a retried request
returns the session it already created instead of launching a second agent. Today that record is kept forever for every
create from the app and for `farhelm agent create`/`clone`, while spawn's records go away with the child. Make every
create's retry key live as long as the session it created, keep records of creates that were refused before any session
existed for 24 hours, and clean up what accumulated so far.

TODO.md entry this plan covers (Near term): "Decide the reservation tombstone scope for interactive creates, then do the
work the verdict leaves standing. When a client attaches an intent key to a create, the supervisor records it in
`create_reservations` so a retried request returns the already-created session instead of double-launching an agent. The
durability-era decision made these reservations PERMANENT for interactive creates (spawn's are session-bounded), which
makes `create_reservations` the only store table that grows without bound — every interactive create ever made adds a
row nothing deletes. Two follow-on debts, deliberately distinct: (a) digest the reservation fingerprint — rows currently
retain enough of the original request to match retries, i.e. request plaintext (titles, cwds, invocations) retained
forever; hashing bounds each row and ends the plaintext retention but does nothing about row count; (b) expiry/pruning —
actually bounds the count, at the deliberate cost that a pruned key becomes reusable after the horizon (a very late
retry could double-create). The digest half is worth doing under either verdict. If the verdict bounds the scope
(session-lifetime — defensible, since a retry outliving the session it protects is protecting nothing), the pruning half
mostly evaporates; that reverses a durability-milestone decision, so record the reversal where that decision lives. The
store module's own docs describe both debts."

Most of debt (a) is already done on main: since 2026-09-30, Delete reduces a session's permanent reservation to a
`sha256:` digest, opening the store does the same for older deleted sessions, and agent create/clone rows store only a
request digest. What remains of (a) is refusal records, whose request text the 24-hour prune now bounds. The verdict on
(b) is session lifetime.

### Acceptance criteria

**Key lifetime.**

- A keyed create from the app (New, Clone, Replace, Replace with, quick launch) and from `farhelm agent create` /
  `farhelm agent clone` gets the session-lifetime scope spawn already uses. Deleting the session removes its reservation
  in the same transaction that removes the session, including a reservation still `pending` when the session is deleted
  (pruned, not settled `created`).
- A retry with the same key after Delete is a new create: it creates a new session (an agent's retry still goes through
  the user's approval card as any new request does), instead of today's conflict "this intent key already created
  session X, which has since been deleted; it will not be recreated under the same key".
- A retry while the session still exists behaves exactly as today (replays the session, reconciles a pending attempt).

**Refusal records** (a create refused before any session existed, including the managed-checkout identity-only refusal):

- They are still recorded, for every keyed create (spawn's refusals, skipped today, included: one rule for all creates),
  so a retry within the window replays the original refusal and the "definitely unaccepted" proof still works within it.
  The outline names the three places that today depend on the row being `permanent` and proposes how refusal rows are
  represented.
- They keep their fingerprint and `error_detail` as written. Nothing is digested at write time: the digest exists so
  that deleting a session removes what is stored about it, a refusal record has no session, and the 24-hour prune
  already ends its retention. SPEC_impl.md says refusal records keep the request for at most 24 hours.
- They are pruned 24 hours after they were written, using the row's existing `created_at` (a refusal settles within
  seconds of the claim, so it is a good enough clock). A new column is a schema change and is outside this plan's agreed
  scope (see Fallbacks). Suggested: prune expired refusal records at the start of each keyed create and in the
  first-open cleanup, with no timer. Rows are only ever added by a keyed create, so that bounds the table, and the
  request whose own record has expired prunes it before it is resolved. The final choice is yours; log it.
- After the 24 hours a retry with that key is treated as a fresh request: it re-validates.

**Existing rows** on the first open by the new version: permanent `created` rows whose session is gone are deleted;
permanent rows whose session still exists are converted to session lifetime; refusal rows follow the 24-hour rule (rows
already older are pruned). No `PRAGMA user_version` bump: an older supervisor must still be able to open the database
(it refuses a newer schema outright), and older builds have understood session-lifetime rows since schema 9. This is a
one-way data change: after it runs, downgrading cannot bring back "that session was deleted" answers for sessions
deleted before the update. The maintainer has been told and accepted this; say it in the changelog fragment and in the
report.

**Cleanup that follows from the verdict:**

- The startup digest sweep (`digest_deleted_sessions_reservations`, store.rs around 5839, which today rereads every
  tombstone on every open) is removed, replaced by the first-open cleanup: once Delete prunes rows and old rows of
  deleted sessions are gone, it has nothing left to do. Delete's digest branch in
  `settle_create_reservations_for_delete` becomes dead once no row with a live session stays `permanent`; remove it too.
- The stale store docs are rewritten once, to describe session lifetime: the `Reservation` docs (store.rs, around
  541-571, which still call the digest "unowned" work), the module docs (around 32-43), the migration comment calling
  the table "immortal" (around 1663-1672), and the delete docs (around 3405-3427).
- No index on `create_reservations(session_id)`: after this plan the table holds one row per live keyed session plus a
  day of refusals, so Delete's scan is small. Log it.

**Spec text:**

- SPEC.md: one sentence under session creation or Delete (Delete reads "removes the session and its stored state",
  around line 1191) saying a create's retry key lasts as long as the session it created, and refusal records 24 hours.
- SPEC.md, managed checkouts (around 1017-1020): "Authenticated reconciliation can return the accepted result or a
  durable refusal that prevents this key from allocating later" becomes a refusal that holds for 24 hours, after which
  the key re-validates as a new request. Keep the rest of that paragraph's behavior.
- SPEC.md, Agent-spawned sessions (around 2314-2320) already says agent create/clone keys get spawn's scoping; make sure
  it now matches the implementation, and drop any ambiguity.
- SPEC_impl.md: the paragraph on agent create/clone key scope (around 2010-2022, "The current implementation remains
  described here until a separate retention change is made") describes the new rule; the reservation digest paragraph
  (around 2144-2150) and protocol 53's "definitely unaccepted" paragraph (around 1764-1771, "has settled `Failed`, and
  has no surviving session row") state the 24-hour window. Record the reversal of the original keep-forever decision
  here, in SPEC_impl.md, with its reason: the decision was made when retry keys were first built (July 2026), to stop a
  retry racing a Delete from re-creating the session; the maintainer chose session lifetime because a key that outlives
  its session protects nothing, a lost reply plus a Delete plus a retry is rare, and an agent's retry still needs
  approval. The `lore/` decision records are frozen and are not edited, and no new lore entry is added.
- A changelog fragment (`kind: changed`; a retry after Delete now creates a new session, plus the one-way cleanup).
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-21), from the planning page:**

- Key lifetime: "Until the session it created is deleted (the rule spawned children already use)", chosen over permanent
  with an age-based prune, the status quo, and session lifetime for app creates only.
- Refusal records: "Keep them, pruned by age", with the comment "24 hours is well above what matters and is tiny storage
  wise", chosen over keeping them forever (the planner's recommendation) and over not recording them.
- Existing records: "Prune records of already-deleted sessions and convert live ones on first start", after being told
  this is one-way.
- Where the reversal is recorded: "In the implementation notes (SPEC_impl.md), next to the description of how retry keys
  work", not as a new lore entry.
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md's upgrade compatibility rules (section "Upgrade compatibility and client scale"): no `user_version` bump here,
  and the one-way cleanup is disclosed. `.agents/devbuild.md` asks a devbuild handoff to mention database changes; the
  report says the cleanup is irreversible on a shared devbuild state.
- `lore/AGENTS.md`: lore is frozen.
- Root `AGENTS.md`, "Changelog fragments", "Tests and validation"; `.agents/test-authoring.md`.

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main (verify them):

- Scope is chosen in `crates/farhelm-supervisor/src/service/handlers.rs` (around 50-75): `CreateAdmission::dedup_scope`
  maps `key_lives_with_session: false` to `DedupScope::Permanent`. Only spawn's create sets the marker; the helm's agent
  relay (`crates/farhelm-helm/src/agent_requests.rs`) sets it only for spawn. Changing the `false` arm starts the policy
  change; the supervisor derives the scope, so no helm or protocol change should be needed. It is not the whole change:
  three places depend on the row being `permanent` today and must change with it.
  - Managed-checkout reconciliation refuses any non-`Permanent` row (`service/core.rs`, around 7309: "this intent key
    belongs to a different create request"). Once managed-checkout creates get session lifetime, the helm's keyed retry
    of a live managed checkout (`inspect_github_checkout`, `agent_requests.rs` around 1679, and the app's own retry)
    would be refused. It must accept session-lifetime rows.
  - The protocol 53 "definitely unaccepted" proof requires `reservation.dedup_scope == claim.dedup_scope` (`core.rs`,
    around 7627), plus `Failed` and no session row.
  - The replay path prunes any `SessionLifetime` row whose session row is absent (`prune_orphaned_bounded_reservation`,
    called around `core.rs` 8644 and 8703). A refusal row has no session row, so with session-lifetime scope it would
    vanish on its first replay.
- Proposed representation of refusal rows (planner proposal): a refusal row takes the claim's scope, which going forward
  is always session lifetime, so there is one scope for all new rows. `record_failed_intent` stops skipping
  session-lifetime keys (store.rs, around 3700-3705, whose comment says the row would otherwise be immortal; the 24-hour
  prune answers that). The orphan prune skips `failed` rows. The protocol 53 equality then holds as written. The 24-hour
  prune predicate is `state = 'failed'` with no session row and `created_at` older than the cutoff. The alternative is
  keeping refusal rows `permanent` and relaxing the equality at 7627; if you pick it, log why.
- The table (store.rs, around 1572-1585, plus `dedup_scope` from schema 9): `intent_key`, `fingerprint`, `state`
  (`pending`/`created`/`failed`), `session_id`, `tmux_name`, `error_kind`, `error_detail`, `created_at`, `dedup_scope`.
- Delete: `settle_create_reservations_for_delete` (store.rs, around 5780) prunes session-lifetime rows and settles and
  digests permanent ones. `prune_orphaned_bounded_reservation` (around 5658) prunes an orphaned bounded row on replay.
  `record_failed_intent` (around 3693) skips session-lifetime keys. `tombstone_fingerprint` (around 5880).
- Retry matching: `resolve_reservation` (`service/core.rs`, around 8612); the deleted-session conflict text is nearby
  (search "has since been deleted").
- The app keeps its key in memory only (`crates/farhelm-ui/src/api.rs`, `mint_intent_key` around 2896) and retries only
  when the user presses Launch again; agents can re-run a keyed command at any time.
- Tests to flip or extend: `crates/farhelm/tests/e2e/create_idempotency.rs`
  (`a_replay_for_a_deleted_session_reports_that_it_is_gone`, around 441, flips; the crash and restart replays near it
  stay); store.rs `deleting_a_session_settles_and_keeps_its_reservations` (around 10527, flips) and
  `deleting_a_spawned_child_prunes_its_bounded_key_and_credential` (around 10563, the template);
  `opening_the_store_digests_deleted_sessions_retry_records`; handlers.rs
  `the_helms_spawn_marker_gives_a_key_the_childs_lifetime` (around 7834, whose table pins the unmarked row as
  `Permanent`); service tests in core.rs around `a_deleted_sessions_retry_record_keeps_only_a_digest`,
  `ordinary_create_refusal_proof_requires_the_matching_failed_reservation` and
  `fresh_reconciliation_of_a_deleted_key_still_checks_identity`. The first-open cleanup needs no `.sql` fixture:
  `opening_the_store_digests_deleted_sessions_retry_records` (store.rs, around 10118) shows the pattern (open, insert
  rows, rewrite them with SQL, delete session rows, reopen, assert).
- History, for context only: `TRIAGE_OUTCOMES.md` has the digest decision
  (`idempotency-fingerprint-keeps-raw-cmdline.md`) and the helm namespace for checkout retry keys; keep this plan
  consistent with their reasoning for refusal records.

Proposed shape (planner proposal), as a linear stack:

1. `fix`: interactive and agent create/clone keys take session lifetime; refusal records for every create are kept with
   a 24-hour prune; the three scope-dependent sites above; Delete and replay paths; Delete's digest branch removed; the
   store docs rewritten; the SPEC.md and SPEC_impl.md text; the changelog fragment.
2. `fix`: the first-open cleanup of existing rows, replacing `digest_deleted_sessions_reservations`, with an
   insert-then-reopen test. Fold into PR 1 if small; keeping it separate makes the one-way data deletion its own
   reviewable step. Remove the TODO.md entry in the last PR.

### Validation

Per root `AGENTS.md`, "Tests and validation", `.agents/validation.md` and `.agents/test-evidence.md`:
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, the supervisor crate's store, handler and
service tests, and the end-to-end `create_idempotency.rs` and agent relay tests
(`crates/farhelm/tests/e2e/agent_relay.rs`: an agent create/clone retried after its session was deleted gets a fresh
approval card instead of a deleted-session conflict), through `scripts/record-test-run.py`. New tests: an interactive
keyed create then Delete leaves no row and a retry creates a new session; a pending row deleted mid-create is pruned and
its retry does not relaunch the deleted session's identity; a refusal row (an app create, a spawn, and a
managed-checkout identity-only refusal) replays within 24 hours, survives its own replay, is pruned after, and the
protocol 53 proof holds within the window; a keyed retry of a live managed checkout still reconciles; the first-open
cleanup converts and prunes correctly. The prune takes its cutoff (or `now`) as a parameter, and tests backdate
`created_at` with SQL as existing store tests do; no new clock plumbing through the store, and no sleeping. Run the
test-sleep checker for changed Rust tests, and `python3 releasing/check-changelog.py format`.

## Fallbacks

No complexity gate. Examples of departures that need the scope reassessment: a schema version bump (a new column, an
index), a protocol change between helm and supervisor, any change to how the app holds or retries its key, or a refusal
record design that drops "a refused create replays its refusal" within the 24 hours.

Decisions to log: how refusal rows are scoped and identified for the prune; where the prune runs; that no index is
added; whether PR 2 folds into PR 1; and the exact SPEC wording.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-create-reservation-retention-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user explicitly chose no-workhorse mode. The user forbids sending any unit of your own decomposition of this work to
a weaker model or a lower effort than your own. The review gate and the scope reassessment review are not units of that
decomposition: they keep the model and effort this file demands for them. The user also forbids invoking, loading or
following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already active when you start,
this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The scope reassessment review runs
at your own model and effort. The sub-agents `plans/AGENTS.md` (Sub-agents this file requires) requires of every plan
(the resume check, and the cold reads of a blocked question and of the report) run as that file says.

### Resource watchdog

Immediately after the resume protocol, start a resource watchdog as a background process (not an agent) and keep it
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

- Split the work into a linear stack of reviewable PRs, one reviewable commit, stable bookmark and draft PR each, using
  the `jjstack` skill. The outline above suggests the slicing; err on the side of bite-sized PRs.
- The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/create-reservation-retention/<nn>-<short-name>` (for example
  `plan/create-reservation-retention/01-session-lifetime`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with changelog fragments per root `AGENTS.md` (Changelog
  fragments): a later PR that extends an earlier PR's still unreleased change edits that fragment rather than adding a
  second one.
- The last code PR removes the TODO.md entry this plan covers (it ends with this plan's `Plan:` reference). TODO.md is a
  shared bookkeeping file: on a rebase conflict there, keep both sides.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, Executing one plan, step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes yourself and address what it finds before moving on. The user
demands exactly this reviewer, and no review swarm: a single fresh-context agent on gpt-6.1-sol at high effort, launched
through `$scode-harness-shellout` when your harness cannot serve that model natively (that skill owns the launch
mechanics). The prompt carries the full charter, because the reviewer has nothing else:

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

When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim, as root
`AGENTS.md` requires.

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Same-model
sub-agents you choose to run get bounded units within the outline; an unapproved new mechanism is outside their task,
and one discovered mid-unit is left unimplemented and reported back to you. Apply the same decision boundary as
Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above. In particular, if the 24-hour prune turns out to need a schema version bump
(no usable timestamp), that is a block, not a decision: it changes the downgrade path the maintainer accepted.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md` (Executing one plan, step 10) only for what
cannot proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of open draft PRs exists that together meets the acceptance criteria above,
each PR passed its review gate, and the last code PR removes the TODO.md entry. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/.agents/executing.md` (Executing
one plan, steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the
watchdog. Never edit `plans/` yourself.
