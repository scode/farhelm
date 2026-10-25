# Fix: show the Codex trust hint for the "Quit" variant of Codex's folder dialog

Written against main at 36c8d7b0 on 2026-10-24. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Investigate the missing Codex trust hint.** On Farhelm 0.31.0, Codex showed its "Folder access" / "Trust this
> folder?" question without Farhelm's prominent reminder that Settings can answer it for managed checkouts. Check the
> current behavior and restore the reminder where it should appear; this is already specified for Codex as well as
> Claude. The reported screen offers "1. Trust and continue", "2. Quit" and "enter continue · esc quit", but both 0.31.0
> and current main recognize only "2. Back to Agent Command Center" and "enter continue · esc back". Account for that
> dialog variant and verify the managed-checkout, setting, dismissal and visible-agent-tab conditions too.

What planning found, from a read of main at 36c8d7b0 (line numbers approximate). None of these files changed between
v0.31.0 and that commit, so 0.31.0 behaves as main does:

- **The cause is the text mismatch.** `CodexReader::trust_prompt` in
  `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs` (around 627-642) requires the joined screen to end with
  `enter continue · esc back` (U+00B7 middle dot) and to contain `2. Back to Agent Command Center`, besides "Folder
  access", "Trust this folder?", "1. Trust and continue" and an open numbered menu (`codex_dialog_may_be_open`, around
  730-759). The reported screen fails both of the first two checks, so `workspace_trust_prompt` stays false and the hint
  never shows. The session still read as waiting, because the waiting reader keys on `enter continue` alone (around
  676).
- The only Codex trust fixtures are `crates/farhelm-supervisor/tests/fixtures/screens/codex/0.159.0/waiting-trust.txt`
  (captured) and `waiting-derived-last-option.txt` (hand-made). Both have the "back" shape. SPEC_impl.md (around
  2423-2428, `SessionInfo.workspace_trust_prompt`) says Codex recognition uses that 0.159.0 capture.
- The installed Codex at planning time was 0.164.0. Its binary's strings contain both the "Back to Agent Command Center"
  label and a ` quit` key hint, plus other folder-dialog choices. That suggests the current Codex can draw both shapes,
  but it was not observed, and what selects between them is unknown.
- **The other conditions look correct**, by a static read: managed checkout (`session_view.rs` around 2693-2702 and
  `crates/farhelm-ui/src/workspace_trust.rs` `show_hint` around 129-138), the per-agent switch read (a failed read hides
  the hint, as SPEC.md requires), the per-agent dismissal and occurrence ×, and the agent-tab condition. They mirror
  Claude's and match SPEC.md (around 1004-1009) and SPEC_impl.md (around 2391-2396). By design, and pinned by
  `e2e/tests/workspace-trust.spec.ts` ("Pi and command launches do not borrow a workspace trust hint"), the hint shows
  only for a Codex **agent** launch, not a command launch declared as Codex.

### Acceptance criteria

- Codex's trust question in the "Quit" shape ("1. Trust and continue", "2. Quit", footer `enter continue · esc quit`)
  sets `workspace_trust_prompt`, so the hint appears under the same conditions as for the "back" shape. The "back" shape
  keeps working.
- Each second choice is accepted only with its own footer ("Back to Agent Command Center" with `esc back`, "Quit" with
  `esc quit`). Every other existing requirement stays: "Folder access", "Trust this folder?", "1. Trust and continue",
  an open numbered menu, and the footer ending the screen.
- A screen fixture of the "Quit" shape exists under `crates/farhelm-supervisor/tests/fixtures/screens/codex/`, captured
  or derived per the decision below, and the fixture-state test (`screen_fixtures.rs`, every fixture reads as its named
  state) covers it.
- Unit tests in `screen_reader.rs`, beside `codex_trust_prompt_matches_captured_dialog_only`: the "Quit" screen matches
  with option 1 and with option 2 selected and with a wrapped question; transcript text after the dialog withdraws it as
  for the "back" shape; mismatched choice/footer pairs do not match; the Claude and Generic readers reject it.
- The conditions named in the entry (managed checkout, switch, dismissal, visible agent tab) are verified by reading the
  code and the tests that pin them (`workspace_trust.rs` `hint_requires_all_current_question_gates`,
  `e2e/tests/workspace-trust.spec.ts`). Running the browser spec is not required, since this change cannot reach the
  browser. The report states what was verified and how. If you find a real bug there, fix it in its own PR with a test,
  as part of this plan.
- SPEC_impl.md's sentence on Codex recognition (around 2427) names both shapes and the fixtures they rest on.
- A changelog fragment under `releasing/changelog.d/` per root `AGENTS.md`, "Changelog fragments" (`kind: fixed`).
- The last code PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-10-24, on the planning page:

- **Fixture: a real capture of the current Codex is authorized; a hand-made derived screen if needed.** Recapturing
  spends real vendor turns and is normally done only when asked (`docs/agent-screen-fixtures.md`). The maintainer asked
  for it here. The maintainer added that historical Codex versions do not matter, so target the installed Codex. If it
  will not draw the "Quit" shape, add a hand-made `*-derived-*` screen built from the captured trust screen, which
  `docs/agent-screen-fixtures.md` allows. The recognition is still added, because the report and the installed binary's
  strings both point at the shape existing.
- **Only the reported shape is recognized.** Other folder-dialog shapes found only as strings in the Codex binary ("Open
  restricted", "Open existing task", "Keep current directory", "continue and create sandbox") stay unrecognized, because
  supporting them would be guesswork.
- **Codex command launches still get no hint**, as today and as the existing browser test pins.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.

## Requirement sources

**The user's request:** the TODO.md entry quoted above, and the decisions above.

**Binding repository constraints:** SPEC.md's trust hint paragraphs (around 984-1009); SPEC_impl.md around 2391-2396 and
2423-2428; `docs/agent-screen-fixtures.md` (capture procedure, derived screens, personal data);
`.agents/real-agent-tui.md` (driving a real agent, never the live install); root `AGENTS.md`, "Code rules" (per-harness
behavior lives in the places `crates/farhelm-supervisor/src/agent_kind/mod.rs` maps), "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md"; `.agents/test-authoring.md`;
`docs/test-sleep-check.md` and `scripts/check-test-sleeps.py`.

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- Express the accepted shapes as a small table of (second choice, footer) pairs in `CodexReader::trust_prompt`, so a
  future variant is one line.
- Bound the capture attempt to one or two tries, then go to the derived screen. The previous Codex trust plan saw no
  trust question at all on Codex 0.163.0, because an already-trusted parent directory hid it.
- A captured screen from a Codex version with no fixture directory yet cannot be committed alone:
  `screen_fixtures_are_well_formed_and_cover_every_live_state` (`agent_kind/screen_fixtures.rs`, around 178-210)
  requires every version directory to hold a working, a waiting and an idle screen. Commit the trust screen together
  with at least one passing working screen and one passing idle screen from the same capture run. If those do not exist,
  use the derived screen instead.
- Capture first, by the cheapest route that can show the dialog. Use `scripts/agent-tui.py` per
  `.agents/real-agent-tui.md` in a fresh scratch directory, starting Codex the way Farhelm's agent launch does without
  the trusted-directory override, and observe without answering when you only need the screen. Or run
  `python3 scripts/capture-agent-screens.py --harness codex`. If the full capture produces a new version directory whose
  other screens fail the fixture tests, do not fix those readers in this plan. Commit only the screens you need (the
  trust screen plus the working and idle screens the rule above requires, if they pass), and record the failing
  scenarios in the report as a follow-up. Read every captured file for personal data before committing, as
  `docs/agent-screen-fixtures.md` says.
- A derived screen goes beside the capture it was derived from, with a `.title` copied from it, and its name says it is
  derived (for example `waiting-derived-trust-quit.txt`).

## Outline

One PR, `plan/codex-trust-hint/01-quit-variant`: the recognizer change, the fixture, the unit tests, the SPEC_impl.md
sentence, the changelog fragment and the TODO.md removal. A bug found in the other conditions gets its own PR below it.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`, choose the smallest set of checks that would
expose a regression, and report checks run, reused and skipped. Expect `dprint check` on the changed files, clippy on
`farhelm-supervisor`, `python -B scripts/check-test-sleeps.py` with the interpreter `docs/test-sleep-check.md` prepares,
and, through `python3 scripts/record-test-run.py`, the supervisor's screen reader, screen fixture and trust ticker tests
(`trust_evidence_reaches_replies_and_resets_with_the_run`). No browser change is expected, because the UI consumes the
same flag.

## Fallbacks

Examples of departures that need the scope reassessment: recognizing shapes beyond the reported one, changing the hint's
UI conditions, showing the hint for command launches, or fixing other readers broken by a new capture.

Decisions to log: whether the fixture is captured or derived and why, the exact Codex version and command line used,
every test changed and why, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-codex-trust-hint-log.md` in the parent directory of the checkout you run in, derived as that section says.
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/codex-trust-hint/<nn>-<short-name>`, as
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
