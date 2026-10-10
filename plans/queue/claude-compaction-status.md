# Show Claude sessions as working while they compact

Written against main at de457dc7 on 2026-10-09. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says, except where this plan's own PRs change them as described below.

This plan has no dependency on another plan.

## The goal

While Claude Code compacts its conversation (its screen shows a spinner line such as
`✻ Compacting conversation… (1m 3s · …)`, sometimes for a minute or more), Farhelm should show the session as Running,
not Idle. Ground the fix in a real captured compaction screen, kept as a fixture so a future Claude release that changes
it is caught.

Acceptance criteria:

- The capture tool, `scripts/capture-agent-screens.py`, gains a compaction scenario for Claude: after the turns it
  already drives, it submits `/compact` and saves the screen while compaction is in progress as
  `working-compacting.{txt,title}`. The step first waits for Claude's input box, because the step before it ends without
  waiting. Its wait predicate is its own and mirrors the reader's widened rule (any words, the ellipsis, then a
  parenthesis followed by a digit); it must not reuse the tool's `CLAUDE_SPINNER` pattern, which requires seconds right
  after the parenthesis and so misses a compaction timer past one minute (`(1m 3s`). `docs/agent-screen-fixtures.md`
  lists the new scenario.
- A full Claude re-capture (`python3 scripts/capture-agent-screens.py --harness claude`, so no Codex turns are spent)
  with the installed `claude` (2.1.296 when this was written) is committed as a new version directory under
  `crates/farhelm-supervisor/tests/fixtures/screens/claude/`, including the compaction screen, per
  `docs/agent-screen-fixtures.md` (D1). The existing `claude/2.1.285/` directory stays.
- The Claude screen reader (`ClaudeReader` in `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`) counts a
  spinner line with a multi-word verb as working: `is_claude_working_line` accepts
  `<glyph> <one or more words>… (<digit>` wherever it accepts a one-word verb today (D2). Everything else about the rule
  stays: the line must sit in the lines above the ruled input box, the ellipsis and the parenthesis with a leading digit
  are still required, and finished-turn lines such as `✻ Cogitated for 26s` (no ellipsis) still read idle. The unit test
  `claude_working_line_is_recognized_by_shape` changes its `"✶ Two words… (2s)"` case to working, and gains cases for
  the captured compaction line and for prose shapes that must stay not-working.
- Every fixture in the new version directory reads as the state its name says
  (`screen_fixtures_read_as_the_state_they_were_captured_in`), and so does every older fixture. If the re-capture shows
  that Claude 2.1.296 drifted in a way that breaks another fixture, fix the reader for it in this plan as long as the
  fix stays within the existing rules' intent; a drift that needs a product decision is a block (Unattended fallback).
- SPEC_impl.md's Claude screen-reading paragraphs (the "Claude shows a spinner line directly above its ruled `❯` input
  box" passage) describe the multi-word rule and name compaction as one case it covers. SPEC.md's Status section needs
  no change unless it now misleads.
- A changelog fragment (`fix`) says Claude sessions now show as working while compacting.
- The last code PR removes the TODO.md entry "Show Claude as working while it compacts." (Near term).
- A linear stack of draft PRs, each having passed the review gate. Not marked ready; not merged.

## Requirement sources

**The user's request (TODO.md, Near term, the maintainer's words):**

"Show Claude as working while it compacts. While Claude compacts its conversation (the screen shows 'Compacting
conversation…' with a running timer and token count, sometimes for a minute or more), Farhelm does not show the session
as active. Make status detection recognize compaction as work, and add a captured compaction screen to the Claude screen
fixtures so it stays covered."

**The user's decisions (2026-10-09):**

- D1. Get the real screen by adding a `/compact` scenario to the capture script and re-capturing Claude, which produces
  a new version fixture set and spends a few vendor turns. If the re-capture surfaces other drift in the new Claude
  version, the plan fixes or reports it.
- D2. Widen the working rule to any multi-word spinner verb, not only the compaction wording, so future multi-word
  spinners are covered too, accepting some false-positive risk.
- D3. Review gate: a fresh-context Opus 5.5 agent at high effort with the general charter, no swarm.
- No-workhorse mode, which `plans/AGENTS.md` requires for every plan.

**Planner proposals** (challenge them through Scope reassessment rather than treating them as requirements):

- One PR for the capture-tool scenario plus the reader change plus the new fixtures, because the fixtures are the reader
  change's test and the scenario is how they are produced. Split only if the diff argues for it.
- Keep every other guard of the working-line rule (position above the input box, ellipsis, digit after the parenthesis)
  as the protection against transcript prose that D2 accepts some risk on.

**What the planner found (at de457dc7).**

- Claude status comes only from the screen. The supervisor's ticker (`service/ticker.rs`) reads each live pane through
  `reader_for(kind)` (`agent_kind/screen_reader.rs`), and `service/status.rs::live_status` maps the reading to a status.
  Claude's hooks (`ClaudeIntegration::hook_argv` in `agent_kind/mod.rs`, a `SessionStart` hook only) report conversation
  identity and never affect status, so no hook interaction needs handling.
- `ClaudeReader::read` finds the ruled `❯` input box (`claude_input_box_rule`), then looks at up to
  `CLAUDE_SPINNER_LINES_ABOVE_BOX` (6) lines above it. A line passing `is_claude_working_line` reads anchored Working;
  none reads anchored Idle. `is_claude_working_line` rejects a verb containing whitespace, so "Compacting conversation…"
  reads as recognized idle, with no fallback and no drift log.
- `TRIAGE_OUTCOMES.md` has the discarded entry `claude-spinner-rejects-multiword.md`, discarded only because no captured
  screen proved the case; this plan supplies that screen. The related discarded entry
  `claude-spinner-window-too-short.md` (a long task list pushing the spinner past the 6-line window) is not in scope.
- Fixtures: `crates/farhelm-supervisor/tests/fixtures/screens/<harness>/<version>/<expected>-<scenario>.{txt,title}`,
  loaded by `agent_kind/screen_fixtures.rs::load_fixtures`, which takes the expected state from the name prefix. Every
  fixture is checked automatically; no list to update. `screen_fixtures_are_well_formed_and_cover_every_live_state`
  requires each version directory to cover every live state, and `screen_fixtures_carry_no_personal_data` checks for
  personal data. `*-derived-*` fixtures survive re-capture; the precedent for a reader change with a fixture is commit
  19696f80 (#1478, `working-derived-background-agents`).
- The capture tool drives Claude with `--model sonnet` in its default permission mode (`drive_claude`), needs
  `.ci-tmux/tmux` (`scripts/build-pinned-tmux-ci.sh`), and runs the fixture tests through the recorder afterwards.
  Compaction of the tool's short conversation may finish quickly; the wait predicate has to poll often enough to catch
  it, and its deadline must allow a slow compaction.

**Binding repository constraints:** root `AGENTS.md`: Agent screen fixtures (the capture tool spends real vendor turns;
this plan is the explicit request to run it), Harness-specific code (Claude-specific rules live in the Claude reader,
never in shared code; read the `agent_kind/mod.rs` module map first), Finishing work (targeted validation through the
recorder; the test-sleep check when tests change), Releases and the changelog (a fragment for `fix`), Reproducing
failures, Sharing the machine, Agent scratch space, The live install is off-limits. `.agents/test-authoring.md` for any
test change.

## Outline

The PR slicing below is a proposal; reshape it if the code argues for it, without churn.

### PR 1: Claude sessions show as working while compacting

`fix:` with a changelog fragment. The capture tool's compaction scenario and multi-word spinner pattern; the Claude
re-capture as a new version directory; the reader's multi-word rule and its unit-test cases; any reader fixes the new
version's fixtures need (D1); the SPEC_impl.md paragraph; `docs/agent-screen-fixtures.md`. Remove the TODO.md entry.

If the re-capture shows drift that needs substantial reader work unrelated to compaction, it may become its own PR below
this one, so each PR stays reviewable.

### Out of scope

Hook-driven status for Claude (a `PreCompact` hook), the six-line spinner window, re-capturing Codex, and any change to
how other harnesses read status.

### Validation

Per root `AGENTS.md` Finishing work, choose targeted checks: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, the supervisor's screen-reader and screen-fixture unit tests through the
recorder (`--tmux none` is fine for these; the capture tool itself needs the pinned tmux), the test-sleep check, and
`python3 releasing/check-changelog.py format`. Run the capture tool with `--harness claude`, exactly once per needed
re-capture; a run that fails to catch compaction gets its predicate fixed rather than repeated runs that burn vendor
turns. Read the captured screens' diff for personal data before committing, as `docs/agent-screen-fixtures.md` requires.

## How to run

### Files outside the repository

Per `plans/AGENTS.md` (Files outside the repository): this plan's working log is
`farhelm-plan-claude-compaction-status-log.md` in the parent directory of the checkout you run in, derived as that
section says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory root
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

On every path, check heartbeat freshness before each new build, test run, capture run or review launch. A dead monitor
or a heartbeat stale for two sample intervals pauses new launches until monitoring is restored; restart a dead watchdog.
An alert is an instruction to act: stop launching work, remove build output and scratch you own, wait for or stop the
job most likely responsible, and resume only when the watchdog reports headroom. Record the watchdog's handle, watched
paths, status path and delivery mechanism in the log, include its state in every handoff note so a resumed session
reconciles or restarts it, and stop it when the plan closes. Do not lengthen the sampling interval to save turns.

### PR discipline

- Use the `jjstack` skill. The stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per
  `plans/AGENTS.md` (Executing one plan, step 6); never another plan's PRs. Bookmarks are
  `plan/claude-compaction-status/<nn>-<short-name>`.
- Keep PRs bite-sized without churn: never add code in one PR that a later PR of this stack deletes. Within this run, if
  a PR needs correcting, restructure it rather than stacking a correction on top; that applies to all of this plan's own
  open PRs, including ones an earlier run built.
- Commit messages and PR titles use Conventional Commits, the type reflecting the user-visible effect. A `feat`, `fix`,
  `perf`, `style` or `revert` PR adds a changelog fragment under `releasing/changelog.d/` in the same commit, per root
  `AGENTS.md` (Releases and the changelog). Validate with `python3 releasing/check-changelog.py format`.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read. Leave
  a PR description empty when the diff and title say everything.
- The last code PR removes the TODO.md entry this plan covers.
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR that changes code, tests or scripts, use the active galaxy-brain skill to delegate a review of
that PR's changes, and address what it finds before moving on. The user demands exactly this reviewer, and no review
swarm: a fresh-context agent on Opus 5.5 at high effort, shelled out to the harness that serves that model when the
executing one cannot reach it natively. The prompt carries the full charter, because the reviewer has nothing else:

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

Before implementing a substantial departure from the outline above (a hook-driven status signal, a compaction-specific
state, changes to the six-line window, changes to how other harnesses read status; these are examples, not a blacklist),
and whenever the same component has needed repeated corrective review rounds, run a fresh-context review through
galaxy-brain with this charter, supplying the user's request and decisions above, this outline, the current diff and the
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
alternatives considered: in particular the exact shape the widened rule accepts and rejects, how the capture scenario
waits for compaction, every drift the re-capture surfaced and what you did about it, and every review finding you
decided not to follow. The user will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them. Remove planner-invented machinery that turns
out unnecessary when the decisions still hold. A material scope expansion, a weakened guarantee, or an omitted required
behavior needs an agreed fallback or the user's decision; a review finding or a log entry is not authorization. If the
work needs such a decision, record the concrete tradeoff and block per `plans/AGENTS.md` (Executing one plan, step 10).
If the capture tool cannot get a compaction screen after the predicate has been fixed once (the vendor login is missing,
or compaction is refused), block with that as the question rather than hand-making the screen. Because the PRs form one
linear stack, later PRs sit on top of a blocked one: finish the PRs before it, record the question, and close the plan
as blocked rather than building past it.

## Done criterion

The plan is complete when the draft PRs of Outline exist as one linear stack, together meet the acceptance criteria
above, and every PR that changes code, tests or scripts passed the review gate. Open, not merged. If a `## Decisions`
section exists, its latest entry must also be satisfied. Then close the plan per `plans/AGENTS.md` (Executing one plan,
steps 11 and 12): deliver its report through the queue script, write a closing entry in its log, and stop the watchdog.
Never edit `plans/` yourself.
