# Stop closes the session's terminal tabs, as a containerized Stop already does

Written against main at 66e77cf4 on 2026-11-07. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Consider closing tabs on stop for ordinary sessions too.** Stopping a containerized session stops its whole
> container, terminal tabs included, while stopping an ordinary session ends only the agent and leaves its tabs running.
> Consider making ordinary sessions match.

The maintainer's answer on the picker: "yeah make it consistent so stop always closes terminal tabs."

How it works today, from a read of main at 66e77cf4 (line numbers approximate):

- **Stop** is `handle_stop_session` (`crates/farhelm-supervisor/src/service/handlers.rs` around 762), which holds the
  session's lifecycle claim throughout (around 792). With the agent alive it calls `stop_live_agent` (`service/sweep.rs`
  around 1947); otherwise it records the classification and reaps with `SweepTarget::AgentOnly` (around 1104-1115, "stop
  leaves tabs running whether or not the agent was alive"), then `container.stop()`. `stop_live_agent` is shared
  verbatim with Restart's pre-relaunch reap (`service/core.rs` around 13167; its comment around 1971-1975 says so), and
  Restart must keep tabs (SPEC.md around 1559-1560), so the tab close belongs in the Stop handler, not the shared
  function.
- **Closing a tab**: `close_tab` (`core.rs` around 15902) takes the lifecycle claim and calls `close_tab_claimed`
  (around 15970), which consumes a `KeyedGuard`; old tmux tabs go through `close_tab_window` (around 16044); host tabs
  through `close_host_tab_claimed` (`core/host_tabs.rs` around 935), which refuses a tab whose host cannot be read
  (around 959-970). Delete's teardown (`service/teardown.rs` around 308-578) is not reusable here: it refuses when tab
  rediscovery fails, and it ends the tmux session.
- **A containerized Stop** ends tabs only because `container.stop()` (`container.rs` around 408) stops the container.
- **Tabs can be opened after Stop**: `open_tab` (`core.rs` around 15331) refuses only when the agent's terminal is gone.
- **Confirmations**: Stop never asks today. SPEC.md (around 1666-1681, confirmed 2026-10-01) says a destructive
  confirmation authorizes only what its prompt said, and when the skip-confirmations setting skips the prompt, the click
  authorizes only what the screen showed; the request carries the matching precondition and the supervisor refuses when
  more is alive. Delete's version is `DeleteGuard` (`crates/farhelm-proto/src/lib.rs` around 2668-2705), computed from
  `session.tabs.len()` (`crates/farhelm-ui/src/status.rs` around 420-428) and carried per target in bulk actions
  (`crates/farhelm-ui/src/list/lifecycle.rs` around 54-60, 135-138). `SessionInfo.tabs` is already in every listing.
- **Replies**: Stop replies with no body (`stop_session` in `crates/farhelm-ui/src/api.rs` around 3418 returns
  `Result<(), String>`). Session Delete reports partial outcomes through `SessionDeleteOutcome.notice`
  (`farhelm-proto/src/lib.rs` around 2764).
- **Wording that promises tabs survive Stop**: SPEC.md around 1518-1520 ("Ordinary sessions keep their terminal tabs
  running"), 1743-1748, 1772-1776, 4423-4425; SPEC_impl.md around 2242-2243 ("tabs keep running across Restart and
  Stop") and 4897-4898 (what Stop's sweep selects); UI comments in `status.rs` around 453-456 and `list/shared.rs`
  around 57-58; the session menu's Stop description (`list/row.rs` around 2802, "Ends the agent and its processes"); the
  bulk Stop tooltip (`list/marking.rs` around 315); the agent stop approval card (`approvals.rs` around 836, 977-985);
  CLI help (`crates/farhelm/src/main.rs` around 245) and agent instructions (`crates/farhelm/src/agent_instructions.rs`
  around 786-787); docs: `website/src/content/docs/docs/agents/agent-wrappers.md` (around 163),
  `website/src/content/docs/docs/using/stop-restart-resume.mdx`, `work-in-a-session.mdx` (around 46),
  `containerized-sessions.md` (around 125-128, no longer an exception), and the "Stop those sessions and close their
  terminal tabs" lines in `manage-hosts.mdx` and `get-started/uninstall.md`.
- **Tests asserting tabs survive Stop**: `crates/farhelm/tests/e2e/terminal_tabs.rs` around 1308
  (`stopping_the_agent_leaves_a_tabs_shell_and_its_daemonized_child_running`), `marker_model.rs` around 28,
  `host_tabs.rs` around 431, 1440 and 1509, `containerized_sessions.rs` around 839, the `AgentOnly` unit test in
  `sweep.rs` around 2441, and `e2e/tests/delete-precondition.spec.ts` (its tabs-only fixtures open a tab, then Stop).
  Restart keeps its tests: `terminal_tabs.rs` around 1403, `host_relaunches.rs` around 287, `core.rs` around 23899.

### Acceptance criteria

- **Stop closes every tab.** After stopping the agent (or reaping it, when it had already ended), Stop closes each of
  the session's terminal tabs, old tmux tabs and terminal-host tabs alike, under the lifecycle claim it already holds,
  so no Restart or other lifecycle action can slip in between. A containerized Stop keeps stopping its container.
- **A tab that cannot be closed** (its host unreadable) does not stop the rest: the agent stops, the other tabs close,
  and the reply reports which tab could not be closed, reusing the notice shape session Delete uses. The page shows that
  notice.
- **Stop asks first when tabs are open:** "Stop the agent and close N terminal tabs?" (or, on a session whose agent has
  already ended, a question about the tabs only), for the row's Stop, the session view's Stop and bulk Stop, skippable
  with the existing skip-confirmations setting.
- **The precondition.** Following SPEC.md's confirmation rule: a Stop whose click showed no tabs (no question asked, or
  the question skipped by the setting while no tabs showed) carries a precondition that the host refuses when a tab is
  open by the time it acts; a Stop whose question named tabs closes every tab, including one opened in between (the
  residual SPEC.md already accepts for Delete). Bulk Stop carries it per target, as bulk Delete does.
- **Agent-requested Stop** (`farhelm agent stop`, approved by the user) is the same Stop and closes tabs; its approval
  card says Stop also closes the session's terminal tabs. That wording authorizes it, so the agent path carries no tab
  precondition.
- **Unchanged:** Restart keeps tabs; the supervisor's own orderly stop, Update and uninstall keep their behavior; an
  agent that exits on its own leaves its tabs running; tabs can still be opened after Stop, and the next Stop or Delete
  closes them; Delete is unchanged.
- **Wording** everywhere listed above says Stop closes the session's terminal tabs: SPEC.md, SPEC_impl.md, the session
  menu's Stop description, the bulk Stop tooltip, the approval card, CLI help, agent instructions, the UI code comments,
  and the docs pages.
- **Tests:** the Rust tests that assert tabs survive Stop are inverted to assert they close (not deleted), for tmux tabs
  and host tabs, with a SIGHUP-immune job under a tab's shell ended; new tests for the unreadable-tab report, the
  precondition refusing a Stop that showed no tabs, a confirmed Stop closing a tab opened in between, and Restart still
  keeping tabs; the browser tests for the question, the skip setting, bulk Stop and the notice; and
  `e2e/tests/delete-precondition.spec.ts`'s tabs-only fixtures reworked to open their tab after Stop. Browser tests on
  Chromium and WebKit.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: changed`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-07:

- **Stop always closes the session's terminal tabs** (on the picker: "make it consistent so stop always closes terminal
  tabs").
- **Stop asks first when tabs are open** ("Stop the agent and close 2 terminal tabs?"), skippable with the
  skip-confirmations setting. The planner's recommendation was no question; the maintainer chose the question.
- **An agent-requested Stop is the same Stop**, and its approval card says tabs close too.
- **A tab the host cannot close**: Stop still stops the agent and closes the other tabs, and reports the tab it could
  not close.
- **Stated on the planning page and not objected to:** Restart keeps tabs running; Update and supervisor restarts keep
  everything running; an agent that exits on its own leaves its tabs running; tabs can still be opened after Stop and
  are closed by the next Stop or Delete; old tmux terminals and new terminals behave the same.
- **The precondition** is required by SPEC.md's confirmation rule (found by the planning review after the questions were
  settled); it follows from the maintainer's choice to ask, not a new product decision.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry, the maintainer's picker note and the decisions above.

**Binding repository constraints:** SPEC.md's Lifecycle (around 1518-1560), session view and tabs (around 1743-1776),
destructive confirmations (around 1645-1681), agent-requested actions (around 3241-3243 for the displayed-state
precedent), and the container section (around 4128, 4423-4425); SPEC_impl.md around 2242-2243, 2315-2318 and 4897-4898;
the per-harness rule in root `AGENTS.md`, "Code rules" (no harness comparisons at call sites); the protocol's
compatibility rules (SPEC.md "Upgrade compatibility and client scale") for the new precondition field and reply notice;
`docs/desktop-web-triage.md`; root `AGENTS.md`, "Tests and validation", "Conventional Commits", "Changelog fragments",
"TODO.md and FLAKES.md"; `.agents/test-authoring.md`; `docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Add a borrowing variant of `close_tab_claimed` that runs under the claim the Stop handler holds, looped over the tabs
  the host finds, rather than re-claiming per tab.
- The precondition is a single flag on the Stop request ("refuse if any tab is open"), set whenever the click showed no
  tabs; an older supervisor ignores it, which is acceptable because an older supervisor never closes tabs on Stop.
- The question reuses the row's existing confirmation helpers (`row_questions`) and the bulk lifecycle's target
  snapshot.

## Outline

Two PRs:

1. `plan/stop-closes-tabs/01-stop-closes-tabs`: the supervisor closing tabs on Stop under its claim, the unreadable-tab
   report in the reply, the precondition field and its refusal, the inverted and new Rust tests, and the UI's question,
   precondition, notice and wording, with the browser tests. The host change and the precondition ship together, so no
   build ever closes tabs on a Stop the user was not asked about.
2. `plan/stop-closes-tabs/02-stop-closes-tabs-docs`: SPEC.md, SPEC_impl.md, CLI help, agent instructions, the docs
   pages, the changelog fragment and the TODO.md removal. Fold the spec text into PR 1 instead if it reads better with
   the behavior.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy, the
pinned nextest and tmux setup and, through `python3 scripts/record-test-run.py`, the e2e selections for terminal tabs,
host tabs, the marker model, host relaunches (Restart keeping tabs) and the containerized sessions suite if its test
changes; the supervisor and proto unit tests touched; `python -B scripts/check-test-sleeps.py` with the interpreter
`docs/test-sleep-check.md` prepares; and the browser specs for Stop, bulk actions and `delete-precondition.spec.ts`, on
both engines. Build the website (`cd website && bun install --frozen-lockfile && bun run build`) for the docs changes,
and follow `website/AGENTS.md` for docs edits (an unattended run starts no preview server and names the changed pages).

## Fallbacks

Examples of departures that need the scope reassessment: closing tabs when the agent exits on its own, changing Restart,
Delete or the supervisor's orderly stop, a tab count on the agent approval card, or a new error class for the
unreadable-tab report.

Decisions to log: the claim-holding close variant, the precondition's shape, how an unreadable tab is reported, every
test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-stop-closes-tabs-log.md` in the parent directory of the checkout you run in, derived as that section says.
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/stop-closes-tabs/<nn>-<short-name>`, as
  the outline names them.
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

## Decisions

### 2026-11-07: answer to a blocked question

The question, as the executor put it:

> Blocked while landing on 2026-11-07 (claim 9db5eb).
>
> ### Stop closing terminal tabs breaks session-list and confirmation tests
>
> This plan makes Stop also close a session's terminal tabs, asking first when tabs are showing; its report says the row
> above the terminal also gained a Stop button. Its drafts, [#3132](https://github.com/scode/farhelm/pull/3132/changes)
> and [#3133](https://github.com/scode/farhelm/pull/3133/changes), have not merged. It was checked together with the
> other plan in this landing, instant-trash-delete, which then passed the same tests without this plan and is landing
> alone.
>
> With this plan on top:
>
> - Two Rust tests that guard the session list's rendering failed (run `1689f6c6-5913-4c2b-a109-28e876bc297c`): an
>   unchanged row re-rendered 65 times across a test's repeated refreshes of the host list, where it should render once,
>   and switching the selected row re-rendered one row more than the two that changed. Both passed without this plan
>   (run `44345c89-c69e-4e73-8faa-582688471320`). Why this plan affects row rendering is not established.
> - Two sidebar tests of where a bottom row's actions menu opens failed on both engines (WebKit run
>   `9b11d18b-32eb-456d-9a23-46309281453a`, Chromium run `0e536f3c-fa3d-4a52-8182-0596188a0e0e`): the menu no longer
>   reaches the position the tests set up. The same tests passed on both engines without this plan, in the runs listed
>   in instant-trash-delete's landing notes.
> - "Skip session confirmations walks row header interrupted card and tab actions" failed on Chromium (run
>   `6c5af047-1e2b-412a-bec1-9032a84e62b8`): one Stop returned a server error because the session's processes could not
>   be cleaned up in time, and the session it left behind made two later tests in that run fail their cleanup. It passed
>   without this plan too, but that is one failure, possibly from load; whether this plan's tab closing causes it is not
>   established.
>
> Should the plan go back to its executor to fix these? That delays Stop's tab closing by an executor round and its
> review. The landing cannot fix them itself: the re-render cause is unknown, and whether the menu tests need the code
> or the tests changed depends on whether the menu's new size is intended. Landing it as it is would ship the re-render
> regression and leave these tests failing on main. I recommend sending it back, with the third failure reproduced in a
> narrow run before it is treated as the plan's.

The maintainer's answer:

The landing monitor answered this on the maintainer's behalf (`plans/.agents/landing.md`, "Answering blocks"): the
landing failed mainly on tests the plan made fail, so the plan goes back to be fixed.

- Fix the session-row re-render regression so that
  `list::row::tests::repeated_parent_refreshes_do_not_rerender_an_unchanged_row` and
  `list::row::tests::a_selection_change_rerenders_only_the_affected_rows` (`crates/farhelm-ui/src/list/row.rs`) pass
  unchanged. They guard real behavior; do not loosen them.
- Make the two sidebar tests of a bottom row's actions menu pass on both engines ("opening the last visible row's menu
  in a scrolled list stays inside the viewport" and "a retained refusal stays inside a bottom-row menu's scrolling
  panel", `e2e/tests/sidebar.spec.ts`). If the menu legitimately grew, adjust the tests' fixture so it still exercises
  the clamp they check, rather than dropping the check.
- Run "skip session confirmations walks row header interrupted card and tab actions"
  (`e2e/tests/session-confirmations.spec.ts`) repeatedly on both engines. If its Stop fails because the session's
  process scope cannot be torn down, find whether closing tabs during Stop causes it, and fix it if so.
- Also, in `crates/farhelm-proto/src/lib.rs`, remove the two lines this plan added to the protocol version's comment
  about why it bumped; that comment's own rule keeps the reasons in the commit and SPEC_impl.md, which already has it.

Run the whole UI crate's tests and the sidebar, session-confirmations and header browser specs on both engines before
delivering again, and rebase onto main first: instant-trash-delete is landing meanwhile. Two docs-screenshot specs
(`e2e/docs-shots/visual-review.spec.ts`, which counts the header's buttons, and
`e2e/docs-shots/stop-restart-resume.spec.ts`, whose callout says a stopped session's terminal stays) also describe the
old behavior; leave them to the next docs screenshot refresh, whose procedure updates shot specs first.

A later change of mind by the maintainer is a follow-up, not a reason to stop the plan.

## Blocked

Blocked while landing on 2026-11-07 (claim 57cb43).

### A Delete test this plan rewrote fails 4 of 12 times, ending a stopped session as `error`

This plan makes Stop also close a session's terminal tabs, asking first when tabs are showing. Its drafts,
[#3132](https://github.com/scode/farhelm/pull/3132/changes) (the change) and
[#3133](https://github.com/scode/farhelm/pull/3133/changes) (its docs), have not merged; the docs PR cannot land without
the change.

The failing test is `e2e/tests/delete-precondition.spec.ts`, whose two cases stop a running session and then check that
a Delete without a confirmation is refused by the supervisor when something is still alive that the screen did not show;
in the second case, with confirmations skipped, a tab is that something. This plan rewrote the test's setup for the new
Stop: because Stop now closes existing tabs, the second case opens its tab after Stop. On trees without this plan, the
spec passed on both engines in each of the four full browser runs this monitor made on 2026-11-07.

With this plan, the second case failed in the full browser run (Chromium, run `88d8738b-6a0d-43a7-a9e6-d455a1b7775f`): a
tab opened right after Stop was missing from the session listing. Three repeats of both cases on each engine
(`--repeat 3`) then failed 4 of 12 attempts (WebKit run `b53ee14b-9906-4d1d-9485-1526f9ff09e2`, Chromium run
`ad453809-f552-4140-91b0-2ac70a453e01`):

- three times, after the test stopped a running session, its status settled as `error` rather than `exited`, in the
  first case too, which opens no tab;
- once more, the tab opened right after Stop was missing from the listing.

Whether the `error` status comes from the new Stop or only from the test's timing is not established; the missing tab
looks like the test reading the listing before it catches up, but that is not established either.

Should the plan go back to its executor to fix this? That delays Stop's tab closing by another executor round. The
landing cannot tell by itself whether Stop is misreporting an ordinary stop. Landing as it is would leave a test that
failed 4 of 12 times on main, and possibly ship a Stop that reports an error for an ordinary stop. I recommend sending
it back.
