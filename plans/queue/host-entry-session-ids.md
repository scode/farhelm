# Session identity bound to the host entry

Written against main at 319b286a on 2026-10-18. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan runs after `agent-spawned-session-rules.md` and `workspace-trust-destination.md`: the queue holds it back
until those plans have landed on main. The maintainer chose that order because both are small and touch the same agent
request code, so this redesign builds on them rather than the reverse.

## The goal

Session IDs are minted by each host's supervisor, and the helm keys them fleet-wide by the bare ID, first claim wins.
Removing and re-adding a host, or adopting a new identity for it, resets that, so a waiting approval card or a GUI
Delete or Replace confirmation shown for one host entry can be sent to a session on another; a dishonest host can
arrange it, because it sees every session ID in the fleet. The maintainer chose the redesign: a session is identified to
the helm, its clients and agents by its host entry's helm-assigned UUID together with the supervisor's session ID, so
nothing a host reports can choose or reuse another entry's prefix, and a reference to a session never reaches a
different host entry. The helm-to-supervisor protocol keeps bare session IDs, so supervisors need no change.

This also executes `create-reply-foreign-id-bypasses-cache-refusal.md`, which the maintainer folded into this outcome
(its Execution field says so), and `approval-session-id-display-collision.md`, folded in at planning because it changes
the approval-card session rows this redesign rewrites.

Carry out these 2 triage outcomes, each recorded under its own heading in `TRIAGE_OUTCOMES.md`, in this order. Read
every entry in full first: its Assessment says what is wrong and which rule settles it, its Decision holds the
maintainer's words, the planned fix and any complexity gate, and its Completion criteria are the acceptance criteria for
that outcome. The redesign comes first; the display-collision outcome's PR sits on top of it, because it renders the
full IDs the redesign introduces.

1. `pending-approval-can-cross-host-entries.md` (`fix spec+code`): Waiting approvals and GUI Delete/Replace consent can
   act on another host entry after session ownership changes. Its PRs also remove
   `create-reply-foreign-id-bypasses-cache-refusal.md` and its index line, and update that entry's Execution field to
   name them.
2. `approval-session-id-display-collision.md` (`fix code`): Approval cards can display distinct session IDs identically.
   Complexity-gated (discard if not cheap); with this plan's full IDs it renders the full ID with the identity
   rendering.

### The complexity gates

Only `approval-session-id-display-collision.md` has a complexity gate: if the identity rendering of full IDs on cards is
not cheap, discard it as its entry says (record Execution as complete with the gate tripped and the reason, remove its
feedback file and index line). The redesign itself has no gate; the maintainer accepted its cost ("costs are fine").

### Acceptance criteria

- `pending-approval-can-cross-host-entries.md`'s Completion criteria, every bullet, as amended by the plan-specific
  answers below.
- `approval-session-id-display-collision.md`'s Completion criteria, applied to full IDs: two IDs that would collide
  today render differently, while answering the card still sends the card's own approval ID (the ledger's wording says
  "the raw ID"; the card answers with its approval ID, not a session ID, so test that).
- Both feedback files of the redesign and the display-collision file are removed with their index lines, and all three
  Execution fields name their change IDs, bookmarks and PRs.
- The redesign's user-visible change is `fix!` with a `kind: breaking` changelog fragment.

## Requirement sources

**The user's decisions (2026-10-18):** each outcome's Decision field records the maintainer's own words and the agreed
scope. Scheduling the 25 outcomes triaged that day as twelve plans, and which outcomes share a plan, was agreed with the
maintainer. Review gate: a single fresh-context gpt-6.1-sol agent at high effort with the general charter, no swarm.
No-workhorse mode, which `plans/.agents/planning.md` requires.

**Plan-specific answers (2026-10-18):**

- A full session ID is `<entry-uuid>:<session-id>`, with the entry UUID in canonical 36-character lowercase form. It is
  split at that fixed length, never at the first colon, because a host-reported session ID may itself contain `:`.
- The helm reports an agent's own full ID: the `--json` envelope's `caller.session_id` becomes the full ID
  (schema_version 7), and `farhelm agent sessions` marks the caller's row. `$FARHELM_SESSION_ID` stays bare (it comes
  from the unchanged supervisor), and the bare-own-ID exception covers using it.
- Agent-facing host IDs switch from the helm's integer host number to the entry UUID, so a host is named one way.
- Pins, seen state and notification marks of a removed and re-added host's sessions start fresh (accepted). So do those
  of sessions the migration cannot attribute to an entry.
- Bare session IDs that agents learned before the update are refused with a pointer to `farhelm agent sessions`;
  SPEC_impl.md "What running sessions hold across versions" names this one exception to its promise that a newer
  `farhelm` accepts the bare IDs that spawn, create and clone printed. The exception covers everything on that section's
  list that this change alters (the printed IDs, host IDs, the `*` marking of the agent's own session or host), not only
  remembered session IDs.
- `approval-session-id-display-collision.md` is folded into this plan: approval cards show and copy the full ID with the
  stricter identity rendering.
- Commit type `fix!` with a `breaking` changelog fragment.

**Binding repository constraints:** the rule each entry's Assessment cites as settling it;
`review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes"; `AGENTS.md`, "Changelog fragments",
"Conventional Commits" and "Tests and validation", with `.agents/validation.md`; `.agents/test-authoring.md` for any
test change. SPEC.md "Upgrade compatibility and client scale", session ownership and "Agent-spawned sessions";
SPEC_impl.md "What running sessions hold across versions"; `.agents/docs-media.md` before touching anything under
`e2e/readme-hero/` (`stage.ts` reads `/api/sessions`).

**Planner proposals:** each entry's planned fix, the PR order above, and the outline below. A planner proposal is not a
requirement: a smaller design that meets the same completion criteria is fine, logged as a DECISION.

## Outline

The planning session sized this at roughly 3 to 5 thousand changed lines, most of them tests; treat that as a rough
calibration, not a target. Facts from its reading of main (verify them):

- Host entries are `hosts.id INTEGER PRIMARY KEY AUTOINCREMENT` in `crates/farhelm-helm/src/store.rs`; no UUID column
  exists. `session_cache` has primary key `(host_id, session_id)` plus a unique index on `session_id` alone, which is
  the fleet-wide uniqueness this removes. `session_seen`, `session_pins`, `session_notification_marks` and
  `preferences.last_selected` are keyed by the bare ID with no host. History tables are already host-scoped. Approval
  state lives only in memory, keyed by the bare asking ID.
- Migrations are one transaction ladder; an older build already refuses a newer schema, so one-way is the normal case.
  The `hosts` table cannot be rebuilt (dropping it cascades), so the new column is added with `ADD COLUMN` and the
  fresh-create DDL must match the migrated DDL byte for byte (an existing test checks it).
- 22 REST routes take `/api/sessions/{id}`; the UI treats `session.id` as an opaque key and percent-encodes it into path
  segments. The supervisor relay passes agent targets through as opaque strings up to 1024 bytes, so full IDs pass
  through unchanged supervisors.
- The supervisor refuses self-delete and self-prompt by bare equality; a full self ID would get past that, so the helm's
  own check must compare the resolved (entry, session) pair with the asker's, or a session deleting itself deadlocks on
  its own request fence. That helm check is load-bearing: an older supervisor's local self-prompt and self-delete
  refusals compare bare IDs, so a full self ID now reaches the helm through them.
- `caller_host_id` is already a string in the agent Hosts, Sessions and Templates replies (`farhelm-proto`). Once it
  carries the entry UUID, the CLI can build the caller's full ID as `<caller_host_id>:<own id>` without a new reply
  field, which keeps the helm-to-supervisor protocol literally unchanged (the supervisor relays these replies typed).
  Prefer that over a new field.

Internally the helm keeps keying per-session state by its existing integer host number plus the session ID; that number
is never reused within a database, so it is as safe as the UUID inside the helm. The entry UUID appears only in the
formatted full ID at the helm's boundaries (REST, events, agent replies, UI). Binding approval cards and GUI Delete and
Replace confirmations to their entry needs no separate mechanism: once routes and approval maps carry the full ID, the
key itself binds them. Do not also build a per-card host check.

Proposed stack (planner proposal; restructure it if a different cut avoids churn):

1. Helm storage: a schema bump adding `hosts.entry_uuid` (backfilled, unique, minted on host add and for the local host
   row, never from anything a host reports); a session-reference type that parses and formats full IDs so bare and full
   strings cannot be mixed in plain strings; per-session state re-keyed to `(host, session)` through each session's
   cached owner, dropping rows with no owner or a contested ID; store APIs taking the pair. The fleet-wide unique index
   stays for now, so behavior does not change yet.
2. REST and UI: full IDs on every session route, session rows and replies (including `parent`), terminal, tunnels,
   uploads and downloads; routing resolves the entry first, then the session on that host only; GUI Delete and Replace
   bound to their entry; a bare ID from a stale browser tab is refused (the skew line already tells the user to reload).
3. Agent path: verb targets are full IDs, with the bare-own-ID exception resolved on the requesting entry (exact
   equality with the asker's bare ID wins over parsing); self checks on the resolved pair; approval maps re-keyed; agent
   host IDs become entry UUIDs; the caller's full ID; JSON schema_version 7; the CLI's existing self-restart warning (it
   compares the target with `$FARHELM_SESSION_ID`) also recognizes the caller's full ID; agent instructions describe
   full IDs.
4. Remove fleet-wide uniqueness and the contested-ID machinery: Create, clone and interrupted-create replies can only
   name a session on the entry that made them; SPEC.md and SPEC_impl.md state the principle and rules, removing the
   accepted contested-ID exception; the changelog fragment; both feedback files removed.
5. The display-collision outcome: approval cards render full session IDs with the identity rendering, plus its test. Its
   ledger entry says the identity rendering keeps "the shortened labels"; the maintainer's later answer that cards show
   and copy the full ID supersedes that where they conflict. Where a card shortens a displayed ID today, the display may
   stay shortened, but what the card copies is the full ID. The identity row always takes a full line; that is
   acceptable.

Each PR carries the spec sentences for what it changes, so the specs never describe a state the code is not in. The
first four PRs are the redesign outcome; its Execution field names all of them. An approval card waiting at the moment
of the update may get no answer (accepted), and the migration cannot be downgraded (accepted).

### Validation

Per `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression in what each PR changes, and report checks run, reused and skipped. `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`; the `farhelm-helm`,
`farhelm-ui`, `farhelm` and `farhelm-proto` unit and integration tests each PR touches through the recorder, narrow
first; the store's migration tests, including one that migrates a database holding pins, seen state and notification
marks for an owned session, an unowned one and a contested one. This change crosses the helm, UI and agent boundaries,
so the browser specs that drive sessions, approvals and agent commands are worth running on Chromium and WebKit through
the recorder after the documented builds, once at the end of the stack (choose the specs that touch `/api/sessions/` or
`--session`; the full suite only if that leaves a concrete gap). The end-to-end agent CLI tests under
`crates/farhelm/tests/` that name sessions. The test-sleep checker when a browser test changes;
`python3 releasing/check-changelog.py format`; `python3 scripts/check-agents-md.py` if the agent instructions or a
heading change.

## Fallbacks

The redesign has no agreed fallback beyond the answers above; the display-collision gate is above.

Examples of departures that need the scope reassessment: any change to the helm-to-supervisor protocol or to
supervisors, a transition period that accepts unambiguous bare IDs, carrying per-session state across a host's removal,
storing the entry UUID in every per-session table instead of the host number, or a UI that shows the entry UUID anywhere
other than the approval cards.

Decisions to log: the session-reference type and where conversion happens, how the migration attributes each table's
rows, the exact refusal wording for bare IDs, how the self checks resolve, the JSON shape (whether `id` itself becomes
the full ID or a new field carries it; prefer `id` becoming the full ID, since every client already treats it as
opaque), and every SPEC.md and SPEC_impl.md passage changed.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-host-entry-session-ids-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
"Agent scratch space", describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs
and the `TRIAGE_OUTCOMES.md` Execution fields) rather than starting over. If it does not exist, this is a fresh start. A
plan that an earlier executor worked on, or that came back from review, also gets the resume check in
`plans/.agents/executing.md`, "Executing one plan" (step 7) before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: the review gate keeps the
model and effort this file demands, and the scope reassessment runs at your own model and effort. The user also forbids
invoking, loading or following `scode-galaxy-brain` or `scode-model-routing` at any point; if galaxy-brain is already
active when you start, this file is the user's express instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering whenever they are likely to finish the work
sooner without costing quality: independent investigations run in parallel, or implementation split across agents.
Whether to use them is your call. Concurrent agents that might otherwise conflict can run in separate jj workspaces
whose results you merge back; you still own the stack and every PR. A sub-agent whose model is not available as a native
sub-agent type, the reviewer included, is launched through `$scode-harness-shellout`. The sub-agents `plans/AGENTS.md`,
"Sub-agents this file requires", requires of every plan (the resume check, and the cold reads of a blocked question and
of the report) are exempt from these rules and run as that file says.

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

- Follow `review_feedback_queue/.agents/execute-outcomes.md`, "Execute triage outcomes": one linear stack in the order
  given under The goal above, with the redesign split into the bite-sized PRs the outline proposes and one PR for the
  display-collision outcome. Each PR carries its outcome's agreed change, the removal of its feedback file and
  `review_feedback_queue/INDEX.md` entry, and its `TRIAGE_OUTCOMES.md` Execution update. Record the jj change ID and
  bookmark in that Execution field before creating the PR, then add the PR's URL (with `/changes` appended) to the same
  change and push it again; no separate bookkeeping PR. Replace the Execution field's current `planned in ...` text
  rather than appending to it.
- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/.agents/executing.md`, "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/host-entry-session-ids/<nn>-<short-name>` (for example `plan/host-entry-session-ids/01-entry-uuid-storage`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`): `fix` for a change users of Farhelm can observe, with a changelog fragment in the same commit; `test`
  for a change only to tests or their fixtures; `docs` for documentation and specification text only; `chore` for
  maintainer tooling and queue bookkeeping. Only a fragment-requiring type adds a fragment.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TRIAGE_OUTCOMES.md`, `review_feedback_queue/`, `TODO.md` and `releasing/changelog.d/` are shared bookkeeping files:
  on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
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
- the acceptance criteria: this file's goal, the user's decisions, the outline, and the outcome's full
  `TRIAGE_OUTCOMES.md` entry, including any complexity gate (ask the reviewer to flag anything beyond the gate).

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
alternatives considered: in particular those named under Fallbacks above, whether each gated outcome met its gate and
why, and every review finding you decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate (a line number, a file name) is corrected to the truth; the plan-specific
agreed fallbacks are under Fallbacks above. If current code or specs invalidate a recorded triage decision (for example,
the finding turns out already fixed or wrong), that is a block: `review_feedback_queue/.agents/execute-outcomes.md`,
"Execute triage outcomes", returns such an item to the user rather than changing the agreed outcome.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan" (step 10) only for what
cannot proceed without the user, after pushing every outcome that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

The report's "things you should know" states the compatibility facts plainly, in product terms: the update path is
unchanged and supervisors from older releases keep working; the helm's database migration is one-way; pins, seen state
and notification marks are dropped for sessions the migration could not attribute; session IDs agents remembered from
before the update stop working and are refused with a pointer to `farhelm agent sessions`; the `--json` schema version
and the meaning of host IDs changed; and an approval card waiting during the update may get no answer.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and all three outcomes' `TRIAGE_OUTCOMES.md` Execution fields name their change IDs, bookmarks
and PRs (the display-collision outcome may instead be recorded as discarded under its gate). Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
