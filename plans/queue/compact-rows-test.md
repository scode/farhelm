# Update browser tests stale since the guarded permission mode

Written against main at 507dbcbb on 2026-10-14. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

This plan has no dependency on another plan.

## The goal

Since the guarded permission mode landed (#1992), the sidebar's session rows describe their agent and permission mark
differently. Each description now names the agent first and, for an agent launch, ends with the command it was launched
with. A launch with no permission choice gets the grey dashed "agent's own setting" mark instead of the green shield.
Two browser tests still expect the old text, so they fail on main on both engines. Bring them up to date. This is stale
test data, not a product bug: the product follows SPEC.md.

### Acceptance criteria

- `e2e/tests/terminal.spec.ts`, "compact rows retain distinct ended and harness glyphs within two characters", passes on
  Chromium and WebKit, with its cases still covering what they cover today. Concretely, as found at planning time
  (verify against the product; the product and SPEC.md win over this list):
  - the permission mark's tooltip, the agent tooltip and the visually hidden text are now the same full description, so
    the exact-match assertion on the permission mark's `data-tooltip` needs the new text for every case;
  - command and legacy rows read `<agent name or "command"> — <permission phrase> — <invocation>`;
  - agent launches read `<agent name> — <permission help>`, then optionally `Custom permission flags: …`, then
    `Launched with: <stored start command>`, separated by blank lines. Match these with an anchored regular expression
    or with `toContainText` on the meaningful parts, rather than copying the long shared caveat verbatim. A regular
    expression is matched against the raw text, blank lines included (unlike a plain string in `toContainText`, which is
    whitespace-normalized), so a pattern spanning them needs `[\s\S]` or the `s` flag;
  - the `default` case (an agent launch with no permission choice) now shows `data-glyph="own-setting"` in `--fg-1`, so
    its expected glyph and the color probe (which only knows `--ok` and `--warn`) need a third branch;
  - the `approve` case's tooltip shows the stored start command (`Launched with: omp`), not the session's invocation, so
    its invocation expectation changes.
- `e2e/tests/omp-composer.spec.ts` has the same stale expectation (`"approve permission mode"` as the row's tooltip and
  hidden text); update it the same way, and confirm it passes on both engines.
- Look for other leftovers of the removed wording across `e2e/tests/` (for example "default permission mode", "approve
  permission mode" as an exact tooltip, or assertions on the permission mark's tooltip alone; a planning-time sweep
  found none outside the two files above, so this is a confirmation). Fix any you find in the same PR when it is the
  same stale-wording cause; anything with a different cause goes into the report as a follow-up instead.
- Assertions keep testing behavior: do not weaken an assertion to a bare "contains something" where the old one pinned a
  specific mode. Prefer the style of the specs that already assert the new wording: `guarded-permissions.spec.ts` (the
  own-setting mark), `custom-permissions.spec.ts`, and the command-row exact match in `terminal.spec.ts` near the "not
  YOLO, as asserted" assertion.
- No product code changes. If a test reveals a real product bug, leave that assertion as it is, describe the bug in the
  report, and fix only the stale data.
- The TODO.md entry ("Update the stale compact-rows browser test.") is removed in the same PR.

Out of scope: any change to the wording itself, to SPEC.md, or to tests failing for unrelated reasons.

## Requirement sources

**The user's request (2026-10-14):** plan this Near term TODO entry. Its text:

> **Update the stale compact-rows browser test.** `e2e/tests/terminal.spec.ts`, "compact rows retain distinct ended and
> harness glyphs within two characters", has failed on main on both engines since the guarded permission mode (#1992):
> it still expects the old permission-mark tooltips, while each tooltip now names the agent first and its command last
> ("Codex — YOLO, as asserted … — codex --yolo"), and a launch with no permission choice is no longer called "default
> permission mode". Update the expected tooltips and descriptions to the current wording; this is stale test data, not a
> product bug, but every run of the terminal browser specs shows two meaningless failures until it is fixed.

**The user's decisions (2026-10-14):** also fix `e2e/tests/omp-composer.spec.ts`, which has the same stale expectation,
and sweep the other browser specs for leftovers of the old wording; review gate is a single fresh-context gpt-6.1-sol
reviewer at high effort, no swarm; no-workhorse mode, which `plans/AGENTS.md` requires.

**Binding repository constraints:** SPEC.md's sidebar permission marks (the amber slashed shield, the green plain
shield, the grey dashed own-setting shield, and that hover and screen-reader text carry the same meaning); the text
builders in `crates/farhelm-ui/src/list/row.rs` (`permission_description`, `agent_badge`, `harness_name`) and
`crates/farhelm-ui/src/launch_composer.rs` (`permission_help`), which are the source of truth for the strings; root
`AGENTS.md` and `.agents/test-authoring.md` for test changes.

**Planner proposals:** matching agent-launch text by anchored regex or meaningful parts instead of verbatim copies, the
sweep's search terms, one PR.

## Outline

One PR, type `test:`, touching only `e2e/tests/terminal.spec.ts`, `e2e/tests/omp-composer.spec.ts` (and any other spec
the sweep finds) plus the TODO.md removal. Read the current text builders in `row.rs` and `launch_composer.rs` first, so
the expectations come from the code, not from this file.

### Validation

Per root `AGENTS.md` "Finishing work": build the binaries and the web UI as it describes, then run the changed specs
through the recorder on Chromium and WebKit (`.agents/narrow-tests.md` has the narrow recipes; selecting the test by
title with `-g` is fine, under the recorder's generic mode, since its strict browser mode refuses project overrides). A
spec file the sweep changed gets the same. Run the test-sleep checker (`python -B scripts/check-test-sleeps.py`, per
root `AGENTS.md` and `docs/test-sleep-check.md`), since browser tests change. `dprint check` on changed Markdown. No
Rust tests: nothing Rust changes.

## Fallbacks

Agreed fallbacks: if a case cannot keep its exact meaning under the new product behavior (for example a glyph that no
launch can produce any more), replace it with the nearest case that tests the same row property and log it.

Examples of departures that need the scope reassessment: changing product code or wording, adding a test helper module
shared across specs, rewriting the test's structure.

Decisions to log: how each changed assertion matches the new text, and anything the sweep found and fixed or deferred.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is `farhelm-plan-compact-rows-test-log.md`
in the parent directory of the checkout you run in, derived as that section says. Resolve it to an absolute path before
you start. Scratch files go in the agent scratch directory root `AGENTS.md` describes, not in the checkout.

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
  `plan/compact-rows-test/<nn>-<short-name>` (for example `plan/compact-rows-test/01-stale-tooltips`).
- Avoid churn: code added in one PR and deleted in a later one means the stack should have been shaped differently.
  Within this run, a PR that needs correcting is restructured rather than corrected on top, and that applies to PRs an
  earlier run built too.
- Commit messages and PR titles use Conventional Commits, type `test`. No changelog fragment: nothing user-facing
  changes.
- The last PR removes the TODO.md entry this plan covers.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, use the active galaxy-brain skill to delegate a review of its changes, and address what it
finds before moving on. The user demands exactly this reviewer, and no review swarm: a fresh-context agent on
gpt-6.1-sol at high effort, shelled out to a harness that serves that model (the harness-shellout skill owns the launch
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
- the acceptance criteria: this file's goal, the user's decisions and the outline.

When a PR changes tests, include the full text of `.agents/test-authoring.md` verbatim, as root `AGENTS.md` requires.

Where you disagree with a finding, decide on the merits and log the DECISION. Do not write a launch command here or in
the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review through galaxy-brain:

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

Reuse an existing review checkpoint when it can answer the question; do not schedule periodic reviews. Apply the same
decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Fallbacks above, and every review finding you decided not to
follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the user's decisions and the repository constraints still hold.
Wording in this file that turns out inaccurate is corrected to the truth; the plan-specific agreed fallbacks are under
Fallbacks above.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the user's
decision; a review finding or a log entry is not authorization. If no fallback covers it, record the concrete tradeoff
and continue with independent work. Block per `plans/AGENTS.md` (Executing one plan, step 10) only for what cannot
proceed without the user.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists that together meets the acceptance criteria above, each PR
passed its review gate, and the last PR removes the TODO.md entry. Open, not merged. If a `## Decisions` section exists,
its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan, steps 11 and
12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.
