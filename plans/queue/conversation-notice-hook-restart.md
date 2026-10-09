# Make the "conversation not learned" notification actionable, and keep it after a supervisor restart

Written against main at fd025c7b on 2026-10-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan. It covers two TODO.md entries, merged into one plan with the maintainer's
agreement because both change the same notification.

## The goal

A minute after the user's first Enter in an agent's terminal, if Farhelm still has not learned which conversation the
agent is in, the session gets a notification. Two things are wrong with it:

1. Its text is the same for every agent and gives the user nothing to act on. Tailor it per agent: say when that agent
   normally reports its conversation and what the user can do.
2. It is only checked for launches the running supervisor started itself. A restarted supervisor that picks up sessions
   still running in tmux has lost whether each launch got Farhelm's conversation hook, so it never checks them. On the
   Mac that is every quit and reopen of the app, and every update. Record that on the session so a restarted supervisor
   checks the launches it picks up.

Reworking the text also exposed a third problem, which the maintainer chose to fix (D6): the one-minute clock starts at
any submitted Enter, including one that answers a dialog such as Codex's "Trust this folder?" prompt, so a user who
answers it and then waits a minute gets the notification before the agent was due to report.

Acceptance criteria:

- An Enter that answers a dialog the agent is showing does not start the one-minute clock; the first Enter that is not
  such an answer does.

- For every agent that can get this notification, its text names the agent, says when that agent normally tells Farhelm
  its conversation (the same per-agent timing Restart's hover text uses), says Restart cannot resume the conversation
  until it does, and suggests the step in D1. It never points at a log.
- After a supervisor restart, a picked-up launch that got the hook is checked like a fresh one, the one-minute clock
  starting at the first Enter after the restart (D3). A launch that did not get the hook, or whose session was stored
  before this change, stays unchecked, as today.
- At most one such notification per launch still holds across restarts.
- SPEC.md "Status" and SPEC_impl.md ("Runtime state", the notification storage section) describe these changes; the "not
  checked at all" gap in SPEC_impl.md is replaced by the next-Enter gap.
- The last code PR removes both TODO.md entries: "Make the "conversation not learned" notification actionable, per
  agent." and "Keep checking for a silent conversation hook after a supervisor restart."
- A linear stack of draft PRs as in Outline, each code PR having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, the maintainer's words):**

- "Make the "conversation not learned" notification actionable, per agent. A minute after the first line sent to an
  agent, if Farhelm still has not learned the conversation, the session gets a notification: "Farhelm has not learned
  which conversation this agent is in, a minute after the first line you sent it, so Restart will not be able to resume
  this conversation." Seen with Codex, it gives the user nothing to act on. Tailor it to the agent the way PR #1618
  tailored Restart's hover text: for Codex, say that its conversation is normally captured when the first prompt is
  submitted, so by now it should have been, and say what the user can do about it."
- "Keep checking for a silent conversation hook after a supervisor restart. A session notification tells the user when
  an agent launched with Farhelm's conversation hook still has not said which conversation it is in a minute after the
  first Enter, since Restart will then be unable to resume it. That check only covers launches the running supervisor
  started itself: whether a launch got the hook is kept in the supervisor's memory, and a restarted supervisor that
  adopts the sessions still running in tmux has lost it, so for those launches the check never fires until the session
  is relaunched. The launches that miss out are the ones not typed into yet, or typed into less than about a minute
  before the restart, when the supervisor restarted; on the Mac that is any quit and reopen of the app, and every
  update. A user who starts a session, quits Farhelm before typing, reopens it and then types gets no bell if the hook
  is broken. Record on the session whether its launch got the hook, so a restarted supervisor can arm the check for the
  launches it adopts. Listed as a possible follow-up in the session-notifications plan's report."

**The user's decisions (2026-10-08):**

- D1. The suggested step: "If you launched {agent} with a custom command, check that it passes on `{farhelm_args}`;
  otherwise, please send feedback from the help (?) menu." The planner's proposed Codex text, which the maintainer
  approved in shape: "Codex normally tells Farhelm which conversation it is in when you submit your first prompt, so it
  should have by now. Until it does, Restart cannot resume this conversation. If you launched Codex with a custom
  command, check that it passes on `{farhelm_args}`; otherwise, please send feedback from the help (?) menu." Not
  chosen: suggesting the user update the agent (Farhelm cannot tell whether the version is the problem). The exact frame
  is the planner's to settle: the shared timing phrase reads "once you submit your first prompt", not "when", and
  "{agent} normally reports its conversation to Farhelm {timing}, so it should have by now." composes cleanly for every
  agent that can get the notification ("a few seconds after it starts", "as soon as it starts", "once you submit your
  first prompt"), where "which conversation it is in a few seconds after it starts" does not. Use that frame; do not
  build a per-agent text table to match D1 word for word.
- D2. Persisting the flag as a new supervisor database column is accepted, including that an older supervisor then
  refuses the upgraded database on downgrade, as was accepted for schema 28. Upgrades must still work normally.
- D3. For a picked-up launch whose first Enter came less than a minute before the restart, the clock starts again at the
  next Enter after the restart. Do not persist the first Enter's time.
- D4. One plan for both entries.
- D6. Codex can show a "Trust this folder?" prompt on launch (screen fixture
  `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-trust.txt`, "enter continue"), and an Enter
  answering it starts the clock today. Offered softer wording (which would still let the notification appear early) or
  not counting such an Enter, the maintainer chose: "Don't count dialog Enter". Keep "so it should have by now".
- D5. Review gate: "gpt-6.1-sol high, no swarm."
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Binding repository constraints:**

- SPEC.md "Status" (each notification says what happened and what the user can do, or what they lose; never a log; at
  most one of each problem per launch), "Durability and resume", and "Upgrade compatibility and client scale" (no change
  may break the upgrade path from v0.23.0 on; alert on loss of state or a missing downgrade path, which D2 settles).
- SPEC_impl.md "Runtime state" (the tripwire item), "The per-launch identity hook", "Report files", and the notification
  storage section (the supervisor decides the text because it holds the specifics, which agent among them; stored rows
  keep their text, a recurrence rewrites it).
- Root `AGENTS.md` "Harness-specific code": read the module docs of `crates/farhelm-supervisor/src/agent_kind/mod.rs`
  first; no `kind == X`, `matches!` over harnesses, or `_` arm in shared code.
- Root `AGENTS.md` (Finishing work; Conventional Commits; Releases and the changelog; Sharing the machine with other
  agents; Agent scratch space; The live install is off-limits), `plans/AGENTS.md` (Executing), and
  `.agents/test-authoring.md` for test changes.

**What the planner verified (at fd025c7b; find code by name):**

- The text is `HOOK_SILENT_TEXT` in `crates/farhelm-supervisor/src/service/notifications.rs`, kind
  `NotificationKind::HookSilent`. Its module docs carry the wording rule. `resume_withdrawn_text(agent)` beside it
  already takes the agent's name.
- The check is `report_liveness_tripwire` and `notify_hook_silent` in `crates/farhelm-supervisor/src/service/capture.rs`
  (`REPORT_WARNING_AFTER`, 65 seconds; `note_first_input`; `submits_a_line`). `notify_hook_silent` already returns
  unless the kind's `restart_readiness()` is `due_by_first_prompt`, so Pi and OMP never get it; Grok never does either,
  because Farhelm injects no hook for it. The kinds that can get it are Claude, Codex and Goose; derive that from the
  per-kind facts, never by naming kinds.
- The per-agent timing lives in `crates/farhelm-proto/src/lib.rs`: `AgentKind::display_name()`,
  `AgentKind::restart_readiness()`, and `RestartReadiness::clause(ReadinessWording::ToTheUser)` ("a few seconds after it
  starts", "as soon as it starts", "once you submit your first prompt", ...). PR #1618 (`c497b0e5`) built Restart's
  hover text from them; the app adds its feedback invitation in `crates/farhelm-ui/src/session_view.rs`
  (`feedback_note`). Claude, Codex and Goose all take `{farhelm_args}` in a custom command.
- Whether a launch got the hook is `RunCells::hooked` (`Arc<AtomicBool>`) in
  `crates/farhelm-supervisor/src/service/core.rs`; its doc says nothing persists it. It is raised in `launch_reserved`
  and `publish_relaunched` from `Spawned { hooked }`, and is always false from `reload_sessions` (the restart path) and
  the `publish_retained_*` paths. `first_input` is in-memory and stays `None` after a reload, which gives D3 for free.
- The closest precedent for a persisted per-launch fact is OMP's launch provenance: `omp_reporter_asset` and
  `omp_launch_program` on `StoredSession` (`crates/farhelm-supervisor/src/store.rs`), written before spawn by
  `record_omp_launch_provenance` (fenced on generation) through `Supervisor::record_launch_provenance`, and cleared by
  `begin_relaunch`. `launch_scoped` is another. `SCHEMA_VERSION` is 28; `apply_schema` holds the migration ladder; a
  test pins `migrated_and_fresh_schemas_agree`; an upgrade test expects schema 28 (`84fad410`).
- Existing tests to extend: in `capture.rs`, `a_silent_hook_notifies_once_unless_the_store_holds_an_identity` (asserts
  the text), `a_late_report_resolves_silent_hook_history_across_supervisor_restart`,
  `a_silent_hook_does_not_notify_for_an_agent_that_reports_after_its_reply`; in `core.rs`,
  `a_relaunch_mints_fresh_hook_cells_even_when_it_keeps_the_identity` and the reload tests; e2e
  `crates/farhelm/tests/e2e/hook_identity.rs::a_report_survives_a_supervisor_restart`.

- The clock is started in `crates/farhelm-supervisor/src/service/connection.rs`, where a confirmed agent-pane input
  frame that `submits_a_line` calls `note_first_input`. `submits_a_line`'s docs say an Enter answering a dialog counts.
- The supervisor already reads Claude's and Codex's screens
  (`crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`, sampled by the ticker on a budgeted round robin): a
  recognized dialog, the trust prompt among them, reads `ScreenState::Waiting`. Other agents fall back to change
  counting, which never reads waiting. The ticker keeps each session's latest classification (start from
  `ticker::ActivitySample` and the session status it produces).

**Planner proposals** are the mechanisms in Outline that the decisions do not fix. Challenge them through Scope
reassessment rather than treating them as requirements.

## Outline

### PR 1: an Enter that answers a dialog does not start the clock

`fix:` with a changelog fragment (kind `fixed`: answering an agent's prompt, such as Codex's "Trust this folder?", no
longer starts the minute after which Farhelm says it has not learned the conversation).

- Where `note_first_input` is called, skip it when the session's latest screen reading says the agent was showing a
  dialog (`ScreenState::Waiting`). Use the reading taken before this Enter arrived, not a capture taken after it, since
  by then the dialog may be gone. This is a shared rule over the screen readers' output, so it needs no per-harness
  branch; agents without a dedicated reader never read waiting and keep today's behavior.
- A stale reading can err only one way: a real prompt typed before the next sample still reads as the dialog and does
  not start the clock, so the next Enter does. That delays the notification rather than making it false, which is the
  direction SPEC.md "Status" requires. Check what else reads waiting (Claude's "Waiting for N background agents" line
  does) and confirm none of it can stand before a first prompt and suppress the clock indefinitely. Log the DECISION if
  you find a cheap way to narrow it; do not add a capture on every Enter to close it.
- Update `submits_a_line`'s and `note_first_input`'s docs and SPEC_impl.md "Runtime state" (the tripwire item).
- Tests: an Enter while the latest reading is waiting does not start the clock; the next Enter, with the reading no
  longer waiting, does; a session with no reading yet behaves as today.

### PR 2: say what to do, per agent

`fix:` with a changelog fragment (kind `changed`: the notification about a conversation Farhelm has not learned now says
when that agent normally reports it and what you can do).

- Replace `HOOK_SILENT_TEXT` with a function of the agent kind that builds the text from `display_name()` and
  `restart_readiness().clause(ToTheUser)` in D1's frame, plus D1's step. Only generic sessions lack those facts, and
  they never get this notification; Claude, Codex and Goose all take `{farhelm_args}` in a custom command. So no
  fallback wording and no per-kind exception is needed.
- `notify_hook_silent` passes the kind's text. Stored rows keep their old wording; a recurrence rewrites it (as
  SPEC_impl.md already says).
- Add the notification to the list of places that use the timing phrase, in `RestartReadiness::clause`'s doc and in the
  `agent_kind/mod.rs` map's "When Restart becomes available" entry.
- Tests: for each kind the code's own eligibility rule admits, the text names the agent and contains no "log" (Grok is
  admitted by that rule though it never gets a hook; its text need only satisfy the same assertions); the existing text
  assertion moves to the new function.
- SPEC.md "Status" and SPEC_impl.md's notification section: say the text is per agent.

Sessions created before launch kinds existed get the hook without `{farhelm_args}`, so for them the custom-command
advice names a placeholder their command never had. That is accepted as part of D1's text.

### PR 3: keep checking after a supervisor restart

`fix:` with a changelog fragment (kind `fixed`: after Farhelm restarts, including on the Mac when the app reopens or
updates, a session whose agent never reports its conversation still gets the notification).

- Schema 29: one additive column on the sessions table recording whether the current launch got the hook, defaulting to
  not hooked, so rows stored before this change behave as today. Carry it on `StoredSession`.
- Write it with its own store call for the current launch (fenced on generation), next to where
  `Supervisor::record_launch_provenance` is called before tmux starts. Do not extend `record_launch_provenance` itself:
  it dispatches per agent and only OMP's arm does anything, while this flag is the same for every agent. Reset it on
  relaunch where `begin_relaunch` resets provenance. `restart_pending_launch` (the retry after an interrupted create)
  deletes and reinserts the row; the insert must write the column. The recovery paths for an ambiguous or refused create
  keep the launch not hooked.
- `reload_sessions` builds `hooked` from the stored value; rewrite its comment that calls a stored value a guess, as
  well as `RunCells::hooked`'s doc. Leave `first_input` and the latch as they are (D3). Decide, and log, what the
  `publish_retained_*` paths should do, by whether they represent a launch whose spawn recorded the flag.
- At most one notification per launch holds for a precise reason: an open notification row blocks a second insert, and a
  resolved one cannot recur because resolving means a conversation was captured. Expect the supervisor's warning log
  line to repeat once per supervisor restart; that is not a bug to fix.
- Update SPEC_impl.md "Runtime state" (replace the "not checked at all" sentence with the next-Enter gap) and its
  storage notes (schema 29; older supervisors refuse it on downgrade, as accepted), and the upgrade test that expects
  schema 28.
- Tests: a reload carries the flag (both values); a picked-up hooked launch with no report notifies a minute after the
  first Enter following the reload; a pre-existing row (column default) stays unchecked; a relaunch rewrites the flag; a
  retried create writes it. Add an e2e restart test only if an existing seam shortens the 65-second wait; otherwise
  service-level tests suffice.
- Remove both TODO.md entries.

### Validation

Follow root `AGENTS.md` "Finishing work": `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo clippy -p farhelm --bins -- -D warnings`, `python -B scripts/check-test-sleeps.py` (per
`docs/test-sleep-check.md`), and the supervisor and proto crates' tests through `scripts/record-test-run.py` (with the
pinned nextest and tmux setup from `docs/test-run-evidence.md`), plus the e2e tests in
`crates/farhelm/tests/e2e/hook_identity.rs` and the upgrade test touched in PR 3. No browser tests unless the app's
rendering of notification text changes. `python3 releasing/check-changelog.py format` for the fragments.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-conversation-notice-hook-restart-log.md` in the parent directory of the checkout you run in, derived as
that section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
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
a notification (in Claude Code, a stdout line of a monitor started with the Monitor tool; elsewhere the harness's
equivalent) when any watched filesystem drops under 10% or under 5 GB free, or available memory under 10%, whichever
comes first, again when the number keeps falling, on recovery, and if the monitor itself fails. A file update alone is
not a notification. Before relying on it, verify delivery with a harmless synthetic alert, and verify failure detection
by killing a throwaway monitor and confirming you are told. If you omit periodic heartbeat checks, also stall a
throwaway monitor without killing it and confirm you are notified within two sample intervals; a monitor cannot detect
its own sampling loop hanging. If any of these notifications is unavailable or unverified, say so in the log and check
the status file at least once a minute.

On every path, check heartbeat freshness before each new build, test run or review launch. A dead monitor or a heartbeat
stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog. An alert is an
instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the job most likely
responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched paths, status
path and delivery mechanism in the log, include its state in every handoff note so a resumed session reconciles or
restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/conversation-notice-hook-restart/<nn>-<short-name>`.
- The stack follows Outline. Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack
  deletes. Within this run, if a PR needs correcting, restructure it rather than stacking a correction on top; that
  applies to all of this plan's own open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entries this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code or tests, use the active galaxy-brain skill to delegate a review of that PR's
changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review swarm: a
fresh-context agent on gpt-6.1-sol at high effort, shelled out to the harness that serves that model when the executing
one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

> Review the changes in this PR for general correctness (bugs, unhandled cases, broken invariants, wrong behavior
> against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and the goal,
> and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the named file,
> most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing and touch no
> VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, a findings file in
the scratch directory, and the acceptance criteria for that PR: its part of Outline, the decisions that apply to it, and
the goal's acceptance criteria. For a PR that changes tests or fixtures, include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires. Where you disagree with a finding, decide on the
merits and log the DECISION. Do not write a launch command here or in the log; the harness-shellout skill owns launch
mechanics. A PR that changes only Markdown gets no review.

### Scope reassessment

Before implementing a substantial departure from the outline above (persisting the first Enter's time, a new per-kind
table or trait for the text, a new notification kind, a test seam for the 65-second wait; these are examples, not a
blacklist), and whenever the same component has needed repeated corrective review rounds, run a fresh-context review
through galaxy-brain with this charter, supplying the user's request and decisions above, this outline, the current diff
and the proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular how the dialog check reads the latest screen state, the exact per-agent texts,
the column name and where it is written, and what the retained-launch paths do, and every review finding you decided not
to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
Because the PRs form one linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the
question, and close the plan as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code or tests passed the review gate. Open, not merged. If a `## Decisions` section
exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11
and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never
edit `plans/` yourself.
