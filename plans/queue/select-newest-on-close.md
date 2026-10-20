# When the open session goes away, open the session visited most recently

Written against main at be722d41 on 2026-10-20. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file says which of their sentences to change.

This plan runs after `host-entry-session-ids.md`. That plan (in flight when this one was written) re-keys session ids
and the helm's last-selected-session preference, which this plan builds on; its `INDEX.md` line says so, and this plan
can only start once that one has landed. Read the landed change before you design the storage.

## The goal

When the open session goes away (deleted with Delete or Cmd+W, deleted from another window or by an agent, or removed
with its host), the session view should switch to the session the user had open most recently before it. Today it
switches to the newest-created session among the rows the sidebar shows, which is rarely the one the user was just
working in, and nothing records which sessions were visited.

TODO.md entry this plan covers (Near term): "**Activate the most recently opened session when another closes.** Closing
a session should leave the most recently opened session selected and active; today it does not, and what actually gets
selected instead is not yet confirmed. Find what selection happens when a session closes, then make the newest session
take over."

What the planning session found today's behavior to be (verify it): a successful delete clears the selection if the
deleted session was open; the automatic pick then refills the empty pane with the remembered selection (the helm-wide
"last selected session" preference) if that row is listed, which it normally is not, since it names the session just
deleted, and otherwise `newest_created_fallback` over the rows the sidebar currently shows. The automatic pick never
writes the preference. "Most recently opened" means most recently visited, per the maintainer (below).

### Acceptance criteria

- **A visit history on the helm.** The helm keeps a short, capped list of the most recently opened session ids, newest
  first, without duplicates, in the existing shared preference row (the one that holds the last-selected session). The
  helm updates it in the same write that records the last-selected session, so no client sends a second request and the
  list's head is always the last-selected session. "Opened" means the same user actions that write the last-selected
  preference today: clicking a row, the quick switcher, creating a session, Replace, and (new, below) the automatic
  pick. Pick a cap (a few dozen is plenty) and log it; ids of sessions that no longer exist are simply skipped at pick
  time and age out under the cap. The history survives reloads and helm restarts and is the same for every client.
  Clients read it with the preference they already read. So that a window that has been open a while also sees other
  windows' visits, re-read the preference when a pick is about to be made (falling back to the local copy if that read
  fails); no polling.
- **The pick.** When the open session goes away, the session view opens the listed session (one the sidebar shows right
  now, so a filter narrows the choice) that comes first in the visit history, excluding the session that went away.
  Exited sessions and sessions on a disconnected host count like any other. When no listed session is in the history,
  fall back to today's newest-created rule. A freshly loaded client uses the same function: the history's head is the
  last-selected session, so "open the last-selected session, else newest-created" becomes "open the first listed session
  in the history, else newest-created", one rule for both paths.
- **Which removals:** every way the client already notices that the open session is gone: its own delete (row menu,
  header, Cmd+W or Ctrl+Shift+Backspace), a delete from another window or by an agent, and removal with its host.
  Replace and Replace with keep selecting the replacement, as today. Closing a terminal tab is not a session closing and
  keeps today's behavior. This plan adds no new way of noticing a removal; where today's detection does not notice one
  (for example a session deleted elsewhere while a filter is applied), say so in the report rather than building new
  detection.
- **The automatic pick is remembered:** it writes the last-selected preference (and so records a visit), so a reload
  lands on the session now open rather than on the newest-created one. Code comments say the fallback never saves its
  pick, to protect a real choice from a cut-off listing; per the planning review that case can no longer reach the
  fallback while the remembered session is alive, so writing the pick is safe. Verify that, and rewrite those comments
  and the matching SPEC_impl.md text rather than only adding the write.
- Removing a session that is not the open one leaves the selection alone, as today. Deleting the last session leaves the
  pane empty, as today.
- Unit tests for the pure choice (history order, exclusion, the filter, the fallback); a helm test for the history
  update (prepend, de-duplicate, cap); a browser test that opens session A, then B, then deletes B and asserts A is open
  (with a third, newer-created session C in the fleet so the old rule would pick C), plus a reload check that the pick
  was remembered. The shared browser suite's reset helpers (`forgetAutoSelect`, `resetPreferences`) also clear the
  history.
- Upgrades: the new preference column goes through the helm's normal schema migration and follows SPEC.md, "Upgrade
  compatibility and client scale" (and its replacement, if the upgrade-epochs work has landed by then). Existing
  installations start with an empty history. The migration is one-way: an older helm refuses the migrated database, as
  with every schema change (see Requirement sources).
- SPEC.md gains the rule (in "Session list" or where SPEC.md describes what a freshly loaded client opens, around the
  sentence that it opens the most recently selected session and falls back to newest-created), and SPEC_impl.md
  describes the storage and the choice.
- A changelog fragment (`kind: changed`); the commit type is `feat`.
- The last code PR removes the TODO.md entry.

## Requirement sources

**The user's decisions (2026-10-20), from the planning page:**

- Meaning: "The session you most recently had open before the one that closed (a visit history)", chosen over
  newest-created and over the list's top row.
- Where the history lives: "On the helm, shared by all windows and kept across reloads", chosen over in-window memory
  only. The planning page noted that this needs a schema change. The capped id list in the preference row is the
  planner's smallest form of that choice (from the planning review); a per-session time stored in the helm's session
  table does not work, because that table is a cache the helm refills per host on every refresh.
- Which closings: "Any way the open session goes away: your delete, Cmd+W, deleted from another window or by an agent."
- Candidates: "Only ones the sidebar shows right now (respects a filter); exited and disconnected ones count."
- Remember the pick: "Yes."
- Review gate and mode are in the run instructions below.

**Binding repository constraints:**

- SPEC.md: what a freshly loaded client opens (the most recently selected session from any client, falling back to
  newest-created from the rows the listing carries); "filtering the list is not deselecting"; the list orders and the
  shared preference that holds the order and the last-selected session; "One GUI at a time" (several GUIs at once are
  best effort); "Upgrade compatibility and client scale".
- SPEC_impl.md: how the preference is stored and "Nothing is kept per client: no localStorage key, no field in
  `desktop-client.json`"; the newest-created fallback's rationale, including picking from a cut-off listing.
- Root `AGENTS.md`, "Changelog fragments" and "Tests and validation"; `.agents/test-authoring.md`.

**Planner proposals (checked by a fresh-context planning review):** the outline below. A planner proposal is not a
requirement: a smaller design that meets the same acceptance criteria is fine, logged as a DECISION.

## Outline

Facts from the planning session's reading of main before `host-entry-session-ids` landed (verify them, and re-read
whatever that plan changed):

- The page owner's `on_removed` clears the selection when the removed session is open (`crates/farhelm-ui/src/lib.rs`,
  around 1819-1823). Deletions noticed through the listing go through `selected_vanished` in
  `crates/farhelm-ui/src/list/view.rs` (around 1536-1551), which only runs on a complete, unfiltered listing.
- Host removal has no dedicated path: it is noticed through the same complete, unfiltered listing check as a delete
  elsewhere.
- The automatic pick is in `view.rs` (starting around 2901, through about 3007): it waits while operations are in
  flight, prefers the remembered selection, handles a cut-off listing, then calls `newest_created_fallback` (around
  387-393). It never writes the preference (around 216-218). The writes of the preference on user selection are around
  `view.rs` 2886, 2406 and 4104, and `session_view.rs` 1636.
- The preference is `last_selected` in the helm's store (`crates/farhelm-helm/src/store.rs`) and
  `crates/farhelm-ui/src/preferences.rs`; each client reads it once and keeps an in-memory copy. Session fields clients
  already receive include `created_at`, `last_activity_at` and `seen_activity_at` (the last is a value, not a time, so
  it cannot order visits). The quick switcher keeps no visit order.
- Tests: `the_fallback_picks_the_newest_created_row_not_the_first_one` in `view.rs`; auto-select on load in
  `e2e/tests/sidebar.spec.ts` and `e2e/tests/sort.spec.ts` (with a stubbed listing); delete in
  `e2e/tests/header.spec.ts` and `e2e/tests/session-action-shortcuts.spec.ts`. Nothing checks which session is selected
  after a delete. Helpers `forgetAutoSelect`, `pinAutoSelect` and `resetPreferences` are in
  `e2e/tests/helpers/fleet.ts`; the browser suite shares one helm, so either stub the listing or make the test's
  sessions the newest in the fleet.

Proposed shape (planner proposal):

- Storage: a history column in the preference row, maintained by the helm whenever the last-selected session is written
  (the remembered launch permissions are a precedent for a preference only the helm writes), delivered with the
  preferences clients already read, and re-keyed in whatever form `host-entry-session-ids` gave session ids.
- The choice: factor a pure function next to `newest_created_fallback` that takes the listed rows, the history and the
  removed id (if any) and returns the pick, and use it both where the pane is refilled after a removal and on load.
- The automatic pick writes the preference through the existing write path.
- Suggested stack: (1) the helm-side history and its delivery, with the migration and helm tests; (2) the UI choice, the
  preference re-read and write, the browser tests, SPEC, SPEC_impl and the fragment, and the TODO.md removal. One PR is
  fine if it reads better.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the helm's store and API tests for the new record and migration, the UI
crate's unit tests, and the selection browser specs named above through the recorder on both engines; the test-sleep
checker for changed browser tests; `python3 releasing/check-changelog.py format`.

## Fallbacks

No complexity gate. Examples of departures that need the scope reassessment: a per-session visit table, a separate
visit-history API or push channel, a client-side history in addition to the helm's, or new detection of removals that
today goes unnoticed.

Decisions to log: the history's cap and column shape, where the pre-pick re-read happens, how a cut-off listing is
handled, and the migration's shape.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md` (Files outside the repository): this plan's working log is
`farhelm-plan-select-newest-on-close-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root `AGENTS.md`
describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks and open
PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier executor worked on,
or that came back from review, also gets the resume check in `plans/.agents/executing.md` (Executing one plan, step 7)
before any work.

### No-workhorse

The user forbids sending any unit of your own decomposition of this work to a weaker model or a lower effort than your
own. The review gate and the scope reassessment review are not units of that decomposition: they keep the model and
effort this file demands for them. The user also forbids invoking, loading or following `scode-galaxy-brain` or
`scode-model-routing` at any point; if galaxy-brain is already active when you start, this file is the user's express
instruction to stop it.

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
  `plan/select-newest-on-close/<nn>-<short-name>` (for example `plan/select-newest-on-close/01-visit-history`).
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
agreed fallbacks are under Fallbacks above.

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
