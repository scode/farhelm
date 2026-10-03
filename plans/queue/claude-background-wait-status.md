# Claude reads as working while it says it is waiting on background work

Written against main at ea5bf905 on 2026-10-02. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this file names a passage the user decided to change.

This plan has no dependency on another plan.

## The goal

When Claude Code's main turn has ended but it shows that it is waiting for background work it launched, Farhelm shows
the session as working, not idle. Today the Claude screen reader reads that screen as idle, on purpose.

The TODO.md entry captured the bottom of a real screen (Claude Code with background reviewer subagents; user, host and
path lines left out), top to bottom:

```
● Agent "correctness-state-lifecycle review" finished · 14m 22s

● The state-lifecycle reviewer is done. [... several lines of the main agent's reply, ending:]
  Once it reports, I'll merge everything and hand the findings to the restater to rewrite for readers who don't know the code.

✻ Waiting for 5 background agents to finish
──────────────────────────────────────────────
❯
──────────────────────────────────────────────
  [user's custom status line]
  ⏵⏵ bypass permissions on · 1 shell · /tasks to see subagents · ← for agents

  ● main
  ○ general-purpose (+3)  Reading SPEC.md desktop Quit section          18m 5s · ↓ 489.8k tokens
```

The leading glyph of the waiting line is Claude Code's animated spinner, captured here as `✻`.

Acceptance criteria:

- A line `<glyph> Waiting for <N> background <words> to finish` within the existing window of lines the reader inspects
  above the ruled `❯` input box reads anchored working, whatever the noun (agents, agent, tasks, shells).
- The line must end at "to finish" (trailing whitespace aside), so a reply line such as
  `● Waiting for 3 background
  agents to finish.` does not match.
- Background work that is only listed (the footer's `1 shell` or `/tasks to see subagents` hints, the `●`/`○` agent list
  under the prompt) changes nothing on its own: with no waiting line, the screen reads as it does today.
- A hand-made test screen of the captured state passes the screen-fixture tests as working.
- The reader's doc comment and SPEC_impl.md describe the rule. The TODO.md entry is removed.

## Requirement sources

**The user's request:** "then also plan "Claude shows idle while background agents run"", for the TODO.md `Near term`
entry "Claude Code reads as idle while it waits on background agents" (quoted in part above).

**The user's decisions (2026-10-02):**

- U1. "if the agent report _waiting_ on a backgroudn agent, it is NOT idle. That said, we mus tnot extend that to also
  "hey there's an agent running or a shell ative", but when it's literally saying its' waiting on abackground work - it
  is not idle."
- U2. Agreed: match "Waiting for N background … to finish" whatever the noun; the footer hints, `1 shell` and the agent
  list alone change nothing.
- U3. Agreed side effects, which follow from a working reading with no extra code: Restart asks before stopping such a
  session, and its last-active time keeps updating so it moves up the recently-active ordering while it waits.
- U4. Review gate: a fresh-context Opus 5.5 reviewer at high effort, told to review adversarially. No review swarm.
- U5. No-workhorse mode: you do all the work yourself (see How to run).

**Binding repository constraints:**

- The reader's doc comment in `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs` (the Claude reader) says:
  "Deliberate choices: background tasks announced at an idle prompt read idle, because the agent is waiting on the
  user's next message, not working on one" (added in #1254). U1 reverses this for the explicit waiting line only.
  Rewrite the sentence; keep its other half (work that is only listed reads idle).
- SPEC_impl.md, Supervisor internals, Runtime state, the Status heuristics bullet: "Claude shows a spinner line directly
  above its ruled `❯` input box for the whole of a turn". Amend it. SPEC.md needs no change: its Status section's "their
  busy indicators" already covers this line.
- `docs/agent-screen-fixtures.md`: hand-made screens are named `*-derived-*` and survive re-capture. Every version
  directory must hold a working, a waiting and an idle screen, and the fixture test requires each screen to read its
  expected state anchored.
- Root `AGENTS.md`: a `fix` PR carries a changelog fragment and removes the TODO entry it addresses. Do not run
  `scripts/capture-agent-screens.py`; it spends real vendor turns and runs only when the maintainer asks.

**Planner choices (from the planning review):**

- P1. The test screen goes in `crates/farhelm-supervisor/tests/fixtures/screens/claude/2.1.285/` as
  `working-derived-background-agents.txt` with its `.title` sidecar. The captured screen's version is unknown, and a new
  version directory holding only this screen would fail the coverage test. Build it from a real 2.1.285 grid (for
  example `idle-after-turn.txt`), with the TODO's lines, redacting the status line the way the other screens do.
- P2. Widen the existing spinner recognition rather than add a parallel mechanism: either extend
  `is_claude_spinner_line` (renamed if that reads better) or add a sibling predicate called in the same place, inside
  the existing `CLAUDE_SPINNER_LINES_ABOVE_BOX` window. The captured line sits directly above the box rule, so no window
  change.
- P3. Tests: add positive rows (the captured line, a singular `1 background agent`, another noun) and negative rows (no
  glyph, no count, the `●` reply with a trailing period) to the existing `claude_spinner_line_is_recognized_by_shape`
  test, and update its docstring for both shapes. Add one unit test that loads the derived screen, replaces the waiting
  line with a finished-turn line (`✻ Cogitated for …`), and asserts anchored idle: that pins U2 without committing a
  second hand-made screen of a state nobody has seen Claude draw. No separate "finished-turn reads idle" test: existing
  coverage has it.

## Implementation outline

One PR (`fix:`). Line numbers drift; find the code by name.

- `screen_reader.rs`: the widened recognition (P2), the rewritten deliberate-choice sentence, and inline comments saying
  why only the explicit waiting line counts (U1).
- The derived screen (P1) and the tests (P3).
- SPEC_impl.md Status heuristics bullet.
- Changelog fragment `kind: fixed`: a Claude Code session waiting on its background agents showed as idle.
- Remove the TODO entry.

### What not to build

No hooks, no new screen state, no capture-script scenario, no change to how the footer or agent list is read, no change
to other harnesses.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-claude-background-wait-status-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
`AGENTS.md` describes, not in the checkout.

### Resume protocol

Your first action is to invoke the `agent-resumeable` skill with the log file's absolute path. If the log exists, read
it and resume where the previous session left off, cross-checking it against reality (the jj graph, bookmarks, open PRs,
TODO.md and `plans/queue/INDEX.md`) rather than starting over. If it does not exist, this is a fresh start. A plan that
an earlier executor worked on, or that came back from review, also gets the resume check in `plans/AGENTS.md` (Executing
one plan, step 7) before any work.

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
  `plan/claude-background-wait-status/<nn>-<short-name>`.
- One commit, bookmark and draft PR. Within this run, if it needs correcting, restructure it rather than stacking a
  correction on top.
- Commit message and PR title use Conventional Commits; the PR is `fix:`. It adds its changelog fragment under
  `releasing/changelog.d/` in the same commit, per root `AGENTS.md` (Releases and the changelog); validate with
  `python3 releasing/check-changelog.py format`.
- Run the commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  the PR description empty when the diff and title say everything.
- The PR stays a draft. Never mark it ready and never merge; landing waits until the maintainer has reviewed this plan's
  report (`plans/AGENTS.md`).

### Validation

Follow root `AGENTS.md` "Finishing work": the smallest set of checks likely to expose a regression, through
`scripts/record-test-run.py` for test execution (with the pinned nextest and tmux setup from
`docs/test-run-evidence.md`). Typical choices, not a checklist: `cargo fmt --all -- --check`,
`cargo clippy
--all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, a focused nextest
selection of the `agent_kind` screen-reader and screen-fixture tests in `farhelm-supervisor`, `dprint check` on changed
Markdown, `python3 releasing/check-changelog.py format`, and `python -B scripts/check-test-sleeps.py` per
`docs/test-sleep-check.md` with `.agents/test-authoring.md` applied, since tests change. No browser tests.

### Review gate

Before finishing the PR, use the active galaxy-brain skill to delegate one review of its changes. The user demands a
fresh-context agent on Opus 5.5 at high effort, reviewing adversarially (U4), shelled out to the other harness if the
executing one cannot reach that model natively. No review swarm. The prompt carries the full charter, because the
reviewer has nothing else:

> Review the changes in this PR adversarially: assume there are defects and hunt for them. Cover general correctness
> (bugs, unhandled cases, broken invariants, wrong behavior against the acceptance criteria quoted below), design
> (whether the shape of the change fits the codebase and the goal, and whether a simpler shape would), and idiomatic
> code for the languages involved. Report findings to the named file, most severe first, each with the file, the quoted
> code, what is wrong, and the smallest fix. Edit nothing and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range, the findings file
(in the scratch directory), and the acceptance criteria: The goal, U1-U5 and P1-P3. Include the full text of
`.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires for test and fixture changes. Address what the
reviewer finds before moving on, and log the DECISION where you decline a finding. Do not write a launch command here or
in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Before implementing a substantial departure from the outline above (a new screen state, reading the footer or agent
list, hooks, a capture-script scenario, a change to the window above the box; these are examples, not a blacklist), and
whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the request, the user decisions above, this outline, the current diff and the
proposed departure (what changed, why it is necessary, and which simpler alternative was ruled out):

> Identify where this plan does more than the user needs. For each substantial new mechanism, explain which requirement
> makes it necessary and whether an existing facility or narrower supported behavior would suffice. Propose the smallest
> coherent design. Challenge planner-authored requirements; identify any simplification that would change an explicit
> user requirement or repository constraint. Report concrete findings and alternatives, or say no unnecessary complexity
> was found. Do not manufacture objections or add safeguards to justify an unnecessary subsystem. Write findings to the
> supplied private file; change no product files or VCS state.

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular the exact match rule, whether the predicate was widened or a sibling added, how
the derived screen was built, the rewritten doc-comment sentence, and every reviewer finding you declined. The user will
ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. Anything
that needs a decision: record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).

## Done criterion

The plan is complete when its one draft PR exists, satisfies the acceptance criteria, has passed the review gate, and
has removed the TODO.md entry. Open, not merged: merging happens only after the maintainer has reviewed this plan's
report. If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/AGENTS.md` (Executing one plan, steps 11 and 12): deliver its report through the queue script, write a closing
entry in its log, and stop the watchdog. Never edit `plans/` yourself.
