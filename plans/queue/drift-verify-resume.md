# Prove Restart resumes the same conversation in harness drift tests

Written against main at 10973c17e on 2026-11-21. This is a goal file for an unattended run, executed through the
planning system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it;
everything you need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over
anything this file says, and where this plan changes what they specify, it says so and you update them in the same
stack.

## The goal

This plan covers one TODO.md entry (bucket "Near term", subsection "Plan follow-up"). Verbatim:

> **Verify Restart actually resumes the same conversation in harness drift tests.** The `harness-drift-tests` Restart
> check currently verifies only that the CLI relaunches with Farhelm's saved resume arguments and accepts fresh
> unsubmitted input in the composer, so it would pass even if an agent silently opened a brand-new conversation instead
> of resuming. Update the drift tests for all eight tracked agents (Claude Code, Codex, Antigravity, Goose, Grok, Muse
> Code, OMP, Pi) to prove the resumed session actually stayed in turn 1's conversation — for example by checking turn
> 1's history or recall and confirming the post-resume hook reports the same conversation ID.

In product terms: Farhelm's opt-in live harness drift tests run real agent CLIs to catch upstream breaking changes.
Today, after Restart, they only check that the agent's prompt box opens and echoes unsubmitted text—which can pass even
if an agent ignored Farhelm's resume flag and started an empty new chat. Update the Restart check across all eight
tracked agents so Turn 1 plants a fresh random code word, Stop + Restart runs, and Turn 2 proves both that the model
recalls Turn 1's code word AND that a new post-resume hook report arrives carrying the same resumable conversation
identity.

### Where it stands (from planning research and planning review at main 10973c17e; line numbers approximate)

- **Drift test implementation (`crates/farhelm/tests/e2e/real_agent_capture/drift.rs`, about 15-21, 494-496, 527-675).**
  Turn 1 currently types `"Reply with the uppercase letters O and K joined together, on a line by themselves."` (82
  chars), waits up to `TURN_LIMIT` (180s) for `"OK"`, `snapshot.captured_conversation.is_some()`,
  `RestartOffer::Resume`, and `"acked"` in `hook-log/<session-id>.log`. Then it calls `stop_session` +
  `restart_session(id, false)`, checks `resumed.generation > captured.generation`, waits for composer readiness
  (`START_LIMIT`, 90s), and types `"Farhelm drift resume input"` without pressing Enter.
- **Prompt length vs. `Terminal::type_text` (`COLS = 115`).** `drift.rs:15` sets `COLS = 115`, and `Terminal::type_text`
  (`drift.rs:494-496`) checks unbroken `text.contains(value)`. Keep both Turn-1 and Turn-2 prompts short enough not to
  wrap inside a 115-column TUI composer (roughly `<= 85` characters), or normalize line breaks when checking echo in
  `type_text`.
- **Reuse `SessionSnapshot::resume_argv` directly for conversation-ID comparison.** `SessionSnapshot`
  (`crates/farhelm-supervisor/src/service/core.rs`, about 2646-2676) already exposes
  `pub resume_argv: Option<Vec<String>>`, computed by `IntegrationSnapshot::filled_resume_argv(captured_conversation)`
  for all 8 tracked kinds (plain ID for Claude/Goose/Muse/Antigravity, `CodexLocator::resume_id` for Codex,
  `GrokLocator::resume_id` for Grok, `SessionLocator::session_file` for Pi and OMP). Comparing
  `captured.resume_argv == post_resume.resume_argv` (both `Some(...)` with `RestartOffer::Resume`) reuses the
  supervisor's exact per-kind resume extraction with zero new production API and zero edits to `real_agent_capture.rs`.
- **Pre-restart hook-log baseline timing.** `Supervisor::restart_session` preserves the previous `captured_conversation`
  across relaunch, and multi-event agents (Claude, Codex, Antigravity, Grok, Muse, Pi, OMP) can emit a late Turn-1
  `Stop`/`agent_end` hook after `"OK"` arrives. Moreover `hook-log/<id>.log` logs `written` and `refused` lines in
  addition to `acked` lines (`hook_report.rs`, about 469-471). Therefore, count **`acked` lines** specifically
  (`line.split_whitespace().nth(1) == Some("acked")`) **after** `stop_session(id)` completes and after a
  `client.list_sessions()` drain, immediately **before** `restart_session(id, false)`.
- **Runner timeout (`scripts/run-harness-drift.py`, about 121; tests in `scripts/test-run-harness-drift.py`).**
  Currently `--timeout 480` seconds per agent; adding Turn 2 (up to another 180s `TURN_LIMIT`) can exceed 480s on slow
  providers and needs a raised default timeout (plus a new timeout assertion in `test-run-harness-drift.py`).
- **Docs (`docs/harness-drift-tests.md`, about 19-21).** Explicitly notes that conversation-identity verification after
  Restart was left as a follow-up; update it to reflect the two-turn recall + post-resume hook check.
- **Stand-in tests & live runs.** Read `.agents/real-agent-tui.md` before relying on any claim about how an installed
  agent behaves in a terminal. `scripts/run-harness-drift.py:20` discovers live cases matching
  `real_agent_capture::drift::drift_([a-z][a-z0-9_-]*): test` under `--ignored --list`, so non-ignored unit tests in
  `drift.rs` (not named `drift_*`) are safe from runner discovery. Antigravity and Grok still require installed hooks on
  the test host for live runs, and Muse Code 1.4.24 currently has a separate open TODO for missing conversation
  reports—do not block on broken/unconfigured upstream CLIs.

### Acceptance criteria

1. **Turn-1 code word + Turn-2 recall + post-resume hook ID check across all 8 tracked agents** (Claude Code, Codex,
   Antigravity, Goose, Grok, Muse Code, OMP, Pi):
   - Generate a fresh per-run random alphanumeric code word (easy for small/fast models to echo accurately, unambiguous
     on screen, e.g., `FH` + random uppercase/digits) rather than a hardcoded word.
   - Keep both prompts short enough not to wrap inside the 115-column TUI composer (roughly `<= 85` chars; e.g., Turn 1:
     `"Remember code word <TOKEN> and reply with only OK."`; Turn 2:
     `"Join R E C A L L, a hyphen, and the code word on one line."` — 58 chars, expecting `RECALL-<TOKEN>` which never
     appeared in Turn 1 or verbatim in Turn 2's prompt, eliminating scrollback and prompt-echo false positives!).
   - Turn 1 verifies `"OK"`, `captured.captured_conversation.is_some()`, `captured.resume_argv.is_some()`,
     `captured.restart_offer == RestartOffer::Resume`, and at least one `acked` line in `hook-log/<id>.log`.
   - Call `stop_session(id)`, drain via `client.list_sessions()`, and **then** record the baseline count of **`acked`
     lines** (`line.split_whitespace().nth(1) == Some("acked")`) in `hook-log/<id>.log` immediately before
     `restart_session(id, false)`.
   - After `restart_session(id, false)`, verify `resumed.generation > captured.generation`, wait for composer readiness,
     send Turn 2 with Enter, and wait up to `TURN_LIMIT` (180s) for:
     1. The expected Turn-2 recall output (`RECALL-<TOKEN>`, matched robustly against minor bullet/casing formatting,
        never false-matching Turn 1 or Turn 2's prompt).
     2. Strictly more `acked` lines in `hook-log/<id>.log` than the pre-restart baseline (capturing both
        startup-on-resume and post-turn-on-resume hooks).
     3. `post_resume.restart_offer == RestartOffer::Resume`, `post_resume.resume_argv.is_some()`, and
        `post_resume.resume_argv == captured.resume_argv` (reusing `SessionSnapshot::resume_argv` directly without
        adding new production supervisor APIs or editing `real_agent_capture.rs`).
2. **Runner timeout & deterministic unit tests (no new fake-CLI subsystem):**
   - Raise the default per-agent timeout in `scripts/run-harness-drift.py` (e.g., from 480s to 660s or 720s, keeping it
     explicit and documented) and add a timeout assertion to `scripts/test-run-harness-drift.py`.
   - Extract the Turn-2 continuity verifier (recall line matching ignoring Turn-1/prompt echo, `acked`-line delta check
     ignoring `written`/`refused` lines, and `resume_argv` equality check) into clean pure helper(s) in `drift.rs` and
     add non-ignored unit tests (not named `drift_*`) covering:
     - pass (recalled code word + new `acked` line + equal `resume_argv`);
     - fail when code word is missing or only appears in Turn 1 / prompt echo;
     - fail when `resume_argv` changed to a different conversation ID after Restart;
     - fail when no new `acked` line arrived after Restart (including when only a `written` or `refused` line arrived).
3. **Live spot-check (best effort, never blocking on unconfigured agents):**
   - Run `python3 -B scripts/run-harness-drift.py` narrowly on whichever tracked agents are installed and authenticated
     on the executing machine (e.g., Claude Code and/or Codex) to confirm the two-turn prompt wording and recall
     detection work cleanly against real TUIs (respecting `docs/harness-drift-tests.md`). Report any agents that yield
     `COULD NOT RUN` or known Muse 1.4.24 failure honestly in the report; do not block on unconfigured vendors.
4. **Docs & bookkeeping:**
   - Update `docs/harness-drift-tests.md` to document the two-turn code-word recall and post-resume hook ID check.
   - Remove the `Verify Restart actually resumes the same conversation in harness drift tests` entry from `TODO.md`.
   - Test-only change (`test:`), so no changelog fragment is required.

## Decisions already made

Settled with the maintainer on the 2026-11-21 planning page:

- **Verification method:** Plant a random code word in Turn 1, Restart, ask for it in Turn 2, AND verify a new
  post-resume hook report carries the same conversation ID (chosen over ID-only without recall).
- **Review gate:** a single fresh-context `gpt-6.2-sol` agent at `high` effort.
- **Release note:** `No release note needed.`
- **No-workhorse mode:** the plans default, stated on the planning page.

## Requirement sources

**The maintainer's request and decisions:** the TODO entry and the decisions above.

**Binding repository constraints:**

- `docs/harness-drift-tests.md` and `.agents/real-agent-tui.md`.
- `.agents/test-authoring.md` for every test added or changed; `python -B scripts/check-test-sleeps.py` when Rust tests
  change.
- Drift cases remain `#[ignore]` and out of default nextest, deflake, CI, and release gates.
- Root `AGENTS.md`, "Conventional Commits", "Tests and validation", "The live install is off-limits", and its
  "Instructions by path" for every path you touch.

**Planner proposals** (from research and the independent planning review):

- Reuse `SessionSnapshot::resume_argv` directly instead of touching `real_agent_capture.rs` or exposing new `agent_kind`
  APIs.
- Extract and unit-test pure verification helpers in `drift.rs` rather than inventing a new fake-CLI/fake-TUI harness.
- Keep everything in one focused PR (`test:`).

## Outline

1. `plan/drift-verify-resume/01-verify-restart-continuity`: update `drift.rs` across all 8 tracked agents to run Turn-1
   code word + post-stop `acked` baseline + Turn-2 recall + post-resume `acked` delta and `resume_argv` match, extract
   and unit-test the pure continuity helpers (non-ignored, not named `drift_*`), raise the per-agent timeout in
   `scripts/run-harness-drift.py` (+ add timeout assertion in `scripts/test-run-harness-drift.py`), update
   `docs/harness-drift-tests.md`, and remove the TODO.md entry.
   `test: verify Restart resumes the same conversation in harness drift tests`.

## Fallbacks

- If a live spot-check shows a particular fast/cheap drift model struggles with the spaced `"R E C A L L"` phrasing,
  tighten the prompt wording while keeping it `<= 85` chars and ensuring Turn 1 / prompt echo cannot satisfy the recall
  matcher; log the wording choice as a `DECISION`.
- If no live agent credentials are available in the executor's environment, rely on the deterministic unit tests and
  report which live harnesses could not run in the closing report; do not block.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-drift-verify-resume-log.md` in the parent directory of the checkout you run in, derived as that section
says, and the watchdog's status file is `farhelm-plan-drift-verify-resume-watchdog.json` beside it. Resolve both to
absolute paths before you start; this file does not create either. Scratch files go in the agent scratch directory that
`AGENTS.md`, "Agent scratch space", describes, not in the checkout.

### Builds and tests

Run builds and tests as `.agents/validation.md` describes, every runtime test through
`python3 scripts/record-test-run.py`, and report checks run, reused and skipped per root `AGENTS.md`, "Tests and
validation". Run `python3 -B scripts/test-run-harness-drift.py`, the new non-ignored `drift::` unit tests in `farhelm`
`--test e2e`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
`python -B scripts/check-test-sleeps.py` as `docs/test-sleep-check.md` prepares it; plus a narrow live
`scripts/run-harness-drift.py --harness ...` spot-check on installed authenticated agents if available. Apply
`.agents/test-authoring.md` to every test you add or change. Never take a state-changing action against the live Farhelm
install on this machine (root `AGENTS.md`, "The live install is off-limits").

### Resume protocol

Your first action, after confirming that the checkout you run in is a checkout of `scode/farhelm` (the nearest Git
worktree or jj workspace containing your working directory, one of whose remotes points at that GitHub repository; if
not, stop, write nothing, and say so), is to invoke the `agent-resumeable` skill with the log file's absolute path. If
the log exists, read it and resume where the previous session left off, cross-checking it against reality (the jj graph,
bookmarks and open PRs) rather than starting over. If it does not exist, this is a fresh start. A plan that an earlier
executor worked on, or that came back from review, also gets the resume check in `plans/.agents/executing.md`,
"Executing one plan" (step 7) before any work.

### Execution mode: no-workhorse

Never send a unit of your own decomposition to a weaker model or a lower effort than your own. Never invoke, load or
follow `scode-galaxy-brain` or `scode-model-routing` during this plan. No-workhorse is the mode
`plans/.agents/planning.md` sets for plans, and the maintainer saw it stated while planning, so it is an explicit
choice: if galaxy-brain is already active when you start, this file is the instruction to stop it.

Sub-agents at your own model and effort are fine, and worth considering when they are likely to finish the work sooner
without costing quality. Concurrent agents that might otherwise conflict can work in separate jj workspaces whose
results you merge back; you still own the stack and every PR. Launch a sub-agent natively, with model and effort set
explicitly where the mechanism allows; when the model you need is not available as a native sub-agent type (the reviewer
below, for instance), `$scode-harness-shellout` supplies the launch mechanics.

The review gate keeps the model and effort this file demands; the scope reassessment runs at your own model and effort.
The sub-agents `plans/AGENTS.md`, "Sub-agents this file requires", requires of every plan (the resume check, and the
cold reads of a blocked question and of the report) are exempt from the rules above and run as that file says: native,
read-only, at your own model.

### Resource watchdog

Right after the resume protocol, and before the first sub-agent, build or test run, start the `scode-build-goal` skill's
watchdog and keep it running for the whole plan. It is the Deno script `scripts/watchdog` in that skill's directory;
resolve its path with `realpath` so it is the same file whichever harness runs you, and read its module docs. Its status
file is the one named under "Files outside the repository" above. Watch, each as a `--path` of `sample`: the root of the
checkout you run in, the home directory, `/tmp`, the directory holding the status file, your scratch directory, and any
jj workspace or build directory you later create on another filesystem (restart the sampler with it added). Sample every
60 seconds, at the script's default alert levels.

- In Claude Code, run `watchdog observe --status <file>` under the Monitor tool, then
  `watchdog sample --status <file> --path <dir>...` under a second Monitor, both at the maximum timeout, then run
  `watchdog check --status <file>`. Every line either prints is a notification. Whenever either is re-armed, stop the
  other and arm both again, observer first, then `check`. An expiry notice is routine; a monitor exit is a monitoring
  failure.
- In Codex, arm `watchdog observe --status <file> --exit-on-event` and then the sampler as two separate runs through the
  `skillette-codex-wake-on-exit` mechanism (load the `skillette` skill and follow that entry), then run `check`. On the
  observer's callback, acknowledge it and arm a fresh observer; after a `STALE`, also stop the hung sampler through its
  handle and arm a new one; then run `check` and act.
- Before starting the real monitors in each session, prove the mechanism on throwaway status files in your scratch
  directory with `--interval 5`: a sampler run with `--synthetic-alert` delivers an `ALERT`; a sampler killed through
  the `pid` in its status file is reported; a sampler run with `--stall-after 2`, watched by its own observer armed
  first, produces `STALE` within two sample intervals. Stop the throwaway runs afterwards. If the mechanism is
  unavailable or fails this, say so in the log, run the sampler as a plain background process, and rely on `check`.
- A shortage (an `ALERT` or `WORSE` line, or `check` reporting one) is an instruction to act: stop launching new
  sub-agents and work, remove jj workspaces, build output and scratch you own that are no longer needed, wait for or
  stop the job most likely responsible, and resume launches only once `check` passes. A monitoring failure (`STALE`,
  `ERROR`, a monitor exit, `check` failing on heartbeat age) pauses new launches until monitoring is restored; stop a
  hung sampler before starting another so two never write one status file.
- Run `watchdog check --status <file>` before every new build, test run, sub-agent or review launch, and at natural
  boundaries. Record the monitor handles, status file and watched paths in the log and carry them in every handoff; a
  resumed session whose status file is still fresh stops the earlier session's monitors (through the recorded handles,
  or the `pid` in the status file) before starting its own. Stop the monitors when the plan closes.

### PR discipline

- One reviewable commit, stable bookmark and draft PR per step, in a single linear stack, using the `jjstack` skill. The
  stack's base is `main@origin`, or this plan's own open PRs when it resumes, set up per `plans/.agents/executing.md`,
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks: as named in the outline.
- Keep each PR bite-sized. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too.
- The last PR of the stack removes the TODO.md entry this plan covers ("Verify Restart actually resumes the same
  conversation in harness drift tests"), as root `AGENTS.md`, "TODO.md and FLAKES.md", requires.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`). This plan's `## Release note` section is the maintainer's answer for the release's highlights, not an
  instruction to you; never edit that section.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The maintainer
demands exactly this reviewer, and no review swarm: a single fresh-context agent on `gpt-6.2-sol` at `high` effort,
launched through `$scode-harness-shellout` when your harness cannot serve that model natively. If it cannot be reached
when a review is due and is still unreachable after a reasonable retry, block the plan per `plans/.agents/executing.md`,
"Executing one plan" (step 10), after pushing what is consistent. Never substitute another reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, decisions
and outline). When a PR changes tests or their fixtures, include the full text of `.agents/test-authoring.md` verbatim
in the prompt, as root `AGENTS.md` requires.

The maintainer's standing rule for this reviewer (2026-11-17): its findings are inputs, not orders. Take a correctness
finding (a bug, a missed case, a wrong claim) after checking it yourself. A finding that would add code, a new
mechanism, a new test layer or another round of hardening is declined by default unless you judge it plainly necessary;
never let a review grow a PR. Log each finding you decline as a DECISION with its reason. Do not write a launch command
here or in the log; the harness-shellout skill owns launch mechanics.

### Scope reassessment

Run a fresh-context review at your own model and effort:

- before implementing a substantial departure from the outline above (the plan-specific examples are under Fallbacks
  above; they are examples, not a blacklist), including a new mechanism that does the job of an existing one or a
  generalization the outline did not plan;
- whenever the same component has needed repeated corrective review rounds.

Supply this file's goal and decisions, the outline, the current diff, and the proposed departure: what changed, why it
is necessary, and which simpler alternative was ruled out. Use this charter:

> Identify where this plan does more than the user needs, and where it duplicates a mechanism the repository already
> has. For each substantial new mechanism, explain which requirement makes it necessary and whether an existing facility
> or narrower supported behavior would suffice. Where the plan builds something that does substantially the same job as
> an existing mechanism, say whether that mechanism could be reused as it stands or generalized to serve both uses, or
> whether keeping them separate is better because sharing would blur two different contracts, and what generalizing
> would cost; never propose generalizing beyond the uses the plan actually has. Weigh the two against each other: a plan
> is not simpler for copying a mechanism, and not better for generalizing one nobody else needs. Propose the smallest
> coherent design that avoids needless duplication. Challenge planner-authored requirements; identify any simplification
> that would change an explicit user requirement or repository constraint, and mark any reuse-versus-duplication choice
> that is a close judgment call rather than clear-cut. Report concrete findings and alternatives, or say no unnecessary
> complexity or duplication was found. Do not manufacture objections, add safeguards to justify an unnecessary
> subsystem, or add abstraction to justify a generalization. Write findings to the supplied private file; change no
> product files or VCS state.

A close reuse-versus-duplication call that stays within the agreed scope is yours to decide after the reassessment, and
to log as a DECISION. Reuse an existing review checkpoint when it can answer the question; do not schedule periodic
reviews. A sub-agent gets a bounded unit within the outline; an unapproved new mechanism is outside its task, and one
discovered mid-unit is left unimplemented and reported back, with checks and incomplete work stated honestly. Apply the
same decision boundary as Unattended fallback below.

### Decision logging

Log each consequential choice in the working log as it happens, under a scannable `DECISION` label, with the
alternatives considered: in particular those named under Planner proposals and Fallbacks above, and every review finding
you decided not to follow. The maintainer will ask for these later.

### Unattended fallback

Resolve routine implementation forks within the agreed scope and log them as DECISIONs. Remove planner-invented
machinery that turns out to be unnecessary, as long as the maintainer's decisions and the repository constraints still
hold. Wording in this file that turns out inaccurate (a line number, a file name, a test name) is recorded in the
working log with the true value; never edit this plan file.

A material scope expansion, a weakened guarantee, or an omitted required behavior needs an agreed fallback or the
maintainer's decision; a review finding or a log entry is not authorization. If no fallback covers it, record the
concrete tradeoff and continue with independent work. Block per `plans/.agents/executing.md`, "Executing one plan"
(step 10) only for what cannot proceed without the maintainer, after pushing every PR that can be finished.

You run unattended, so never ask the maintainer anything in conversation: no question tool, no approval prompt, and no
"should I ...?" or offer of optional extras closing a turn, whatever a loaded skill or the harness suggests. Every fork
is either decided by you, taking the option that best fits this file, its Decisions and the specs, and logged as a
DECISION, or it is a block per `plans/AGENTS.md`. Things the maintainer should know and possible follow-ups go into the
report's "things you should know" and "open questions and possible follow-ups" sections, never into a question.

## Done criterion

The plan is complete when a linear stack of draft PRs exists, one per step in the order the outline gives (or as the
outline's fallbacks allow), together meeting the acceptance criteria above, each having passed its review gate, and the
last removing the TODO.md entry this plan covers. Open, not merged. If a `## Decisions` section exists, its latest entry
must also be satisfied. Then close the plan per `plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12):
deliver its report through the queue script, write a closing entry in its log, and stop the watchdog. Never edit
`plans/` yourself.

## Release note

No release note needed.
