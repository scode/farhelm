# Remove tmux from test tooling, CI, the brick test and agent instructions

Written against main at 683a9a72 on 2026-11-11. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

Its TODO.md source is the Near term entry "Finish removing tmux and xterm.js after old terminals end". This is the third
of four chained plans that together remove tmux and xterm.js from Farhelm ("phase 2"). It runs after
`tmux-end-old-terminals.md` (the running product: a startup shutdown of the private tmux server, no tmux driver, no byte
streams, no xterm.js, no test that asks for an old terminal) and `tmux-drop-from-install.md` (no tmux in setup, the
desktop app, provisioning, uninstall, the installer or release payloads) have landed. After it comes
`tmux-drop-from-docs.md` (specs and docs not already updated, a comment sweep and a final audit). Read the earlier
plans' reports (`plans/REPORTS.md`, or `git log` for their PRs) to see what they actually removed; whatever they already
took from the lists below is done.

## The goal

### The maintainer's request, in their words

> plan the work to completely nuke tmux/xterm.js support.

> the intent is to have no trace of them. unless you find something that seems like it's worthy of my attention and is
> somehow needed. we want to not find in 2 months there's some crazy complexity somewhere due to tmux/xterm.js still

> this phase should remove all the tmux installation stuff, bundling of tmux in the installation/distribution stuff, all
> the release stuff having to do with it, musl binaries, all the complexity every where except the bare minimum to
> handle safely and cleanly turning off the tmux the supervisor created.

The only tmux left in the product is the startup shutdown the first plan added; its tests need a real tmux server built
in the old layout. A tmux the USER runs inside a session is not Farhelm's and its handling stays (for example
`scripts/agent-tui.py` and `scripts/agent_screen_host.py` stripping `TMUX`/`TMUX_PANE`, and the test
`a_scoped_stop_leaves_jobs_in_a_session_started_stock_tmux_server`).

### Acceptance criteria

1. **No pinned test tmux.** `scripts/build-pinned-tmux-ci.sh`, `scripts/build-private-tmux.sh`,
   `.github/release/ziglang-requirements.txt` and `.github/release/source-pins.env` (and every reader of them:
   `scripts/record-test-run.py`, `scripts/plan-test-hunts.py`, `scripts/desktop-smoke.sh`, anything else), the
   `.ci-tmux` entries in `.gitignore`, and every `PATH="$(scripts/build-pinned-tmux-ci.sh):$PATH"` setup go. Tests that
   need a real tmux (the startup shutdown's, the stock-tmux user test) use whatever tmux the machine has; the release
   workflow's test job installs the distribution's tmux for them, and so does any other CI job that runs them (check the
   macOS jobs' selections). If no tmux is present, those tests skip with a visible `SKIPPED` message the recorder output
   shows, as other substrate-dependent tests do; a skip is not evidence of a pass (root `AGENTS.md`), so a gate that
   runs them must provide tmux. `vendor/README.md` cites the pinned-tmux files; reword those citations with them, per
   `vendor/README.md`'s rules (text only).
2. **The recorder has no tmux mode.** `--tmux {warn,required,none}`, `capture_tmux`, the manifest's `tmux` field and its
   summary go (`scripts/record-test-run.py`, `scripts/test-record-test-run.py`, `scripts/test_run_summary.py`, the
   hunts' hard-coded `"tmux": "required"` in `scripts/hunt-rust-tests.py`, `scripts/hunt-browser-tests.py`,
   `scripts/test_hunt.py`, `deflake/bin/deflake`'s `Phase.tmux` and pinned-tmux preparation, `deflake/SPEC.md`). Every
   documented command passing `--tmux` is swept in the same change: `.agents/*.md`, `docs/test-run-evidence.md` (its
   "Tmux evidence" section included), `releasing/AGENTS.md` (its word-for-word command),
   `releasing/mac-vm-test/OPERATIONS.md`, `e2e/grid-client/BENCHMARK.md`, scripts, and the workflows. Frozen material
   (older FLAKES.md entries, `plans/`, `lore/`, TRIAGE_OUTCOMES records) keeps its old commands. FLAKES.md's header and
   its twin in `.agents/test-evidence.md`, "FLAKES.md", stop asking for a tmux version and SHA-256.
3. **CI and the release workflow** have no pinned-tmux steps, and no step name or comment mentions the pinned build
   (`.github/dist-build-setup.yml`, then regenerate `release.yml` per `releasing/AGENTS.md`;
   `.github/workflows/ci.yml`). The step installing the distribution's tmux for the shutdown tests (criterion 1) is the
   exception and says what it is for. `releasing/rehearse-release-gate.py` requires the literal step name "Run the test
   suite against the pinned tmux"; update it with the rename.
4. **Other test tooling:** `.config/nextest.toml`'s tmux test group and thread reservations whose tests are gone;
   `scripts/tensorlake/*` shipping `.ci-tmux`; `scripts/test-start-stack-cleanup.sh`'s decoy tmux server (keep the
   test's point, that a stack's cleanup spares other stacks, with a decoy that is not tmux, if the stack still kills
   anything by socket; otherwise drop the leg); `scripts/check-tmux-cutover.py` (cited only by SPEC_impl.md);
   `scripts/capture-agent-screens.py`'s tmux-era rationale; `farhelm-teststate`'s tmux module beyond what the startup
   shutdown's tests and the stock-tmux test use. Keep its dead-run sweep that ends tmux servers left in dead `fh-*` run
   directories (`crates/farhelm-teststate/src/lib.rs`, `reap`, and its test `sweep_kills_a_real_tmux_server`): the
   remaining tests still start real tmux servers, and a test process killed before its teardown would otherwise strand
   one; re-document it in those terms. `farhelm-fixtures`' fake-agent modes framed around tmux (`Flood`, `FloodGated`,
   `FloodMemory`, `Counter`) where they have no user left, otherwise re-documented in their own terms.
5. **The brick test** (`releasing/mac-vm-test/`) no longer branches on terminal kind. The first plan already changed the
   reads of a terminal kind from the session details (and removed review-queue item
   `brick-test-requires-hidden-terminal-kind-details`); retire what remains: the "old session in tmux" branch and the
   database terminal-kind checks (`BRICK-TEST.md`, `RECIPE.md`, `brick_test.py`, `checkpoint_contract.py`,
   `guest_operations.py`, `guest_workflow.py`), and the base images' tmux text (`BRING-UP.md`, `OPERATIONS.md`). The
   upgrade promise stays as for any release; the startup shutdown is covered by the Rust tests, as the maintainer
   decided.
6. **Agent instructions** say nothing about tmux beyond the user's own and the kept startup shutdown: root `AGENTS.md`
   (the "dx and tmux-asset builds" note, and the word-for-word exception for "the pinned nextest and tmux `PATH` setup",
   which becomes the pinned nextest setup alone; change every copy, including `.agents/machine.md`, together),
   `.agents/validation.md`, `.agents/run-all-tests.md`, `.agents/narrow-tests.md` (its stale "~330 tests driving a real
   supervisor and real tmux"), `.agents/tensorlake.md`, `.agents/machine.md` (private tmux sockets, `.ci-tmux/`),
   `.agents/test-evidence.md`, `releasing/.agents/build-outputs.md` and `releasing/.agents/rc.md`, `deflake/` docs, and
   `review_feedback_queue/FILTER.md` (the filter "Farhelm's own tmux-based terminals" goes, keeping its carve-out for
   the user's own tmux as a statement of its own if still useful; the tmux half of "A hung private tmux server or
   systemd user manager" goes, keeping the systemd half). `.agents/run-all-tests.md` and `.agents/validation.md`
   disagree about whether the appearance smoke reads xterm.js; settle it to the truth. `plans/.agents/close-reset.md`
   mentions `.ci-tmux/`; plan PRs never touch `plans/`, so leave it and name it in the report.
7. Each PR leaves main coherent, and nothing is added that a later PR of this plan deletes.

## Decisions already made

Settled by the maintainer while planning on 2026-11-11:

- **Tests use whatever tmux the machine has**, not a pinned build. The pinned build, zig pin, tmux source pins and the
  recorder's `--tmux` mode go, every documented command swept in one change; frozen material is left as it is.
- **Brick test:** retire the tmux branch and the terminal-kind checks; rely on the Rust tests for the shutdown.
- **Reviewer:** a single fresh-context agent on Opus 5.5 at high effort (see Review gate). If it cannot be reached when
  a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default.
- **Size gate:** about 900 added lines across the stack (code, tests, scripts and docs; deletions not counted). Over it,
  block and bring it back.

## Requirement sources

**The user's request:** the quotes and decisions above.

**Binding repository constraints:** root `AGENTS.md` ("Frozen material", "Tests and validation", "Conventional Commits",
"Changelog fragments", the Claude Code shell rule, "Reading these instructions" on word-for-word copies, and
"Instructions by path": `releasing/AGENTS.md` for `releasing/`, `dist-workspace.toml`, `.github/dist-build-setup.yml`
and the generated `release.yml`; `deflake/AGENTS.md` for `deflake/`; `review_feedback_queue/AGENTS.md` for `FILTER.md`
and queue removals; `.agents/validation.md` for scripts, workflows and tests); `.agents/test-authoring.md` for test
changes; `vendor/README.md` for its text. Any change to a workflow, check or test script updates
`.agents/validation.md`'s inventory and `.agents/run-all-tests.md` in the same change. Any instruction-file change runs
`python3 scripts/check-agents-md.py` (root `AGENTS.md` stays under its 28 KiB budget).

**Planner proposals** (change them if the code says otherwise, logged as a DECISION):

- The recorder's manifest loses the `tmux` field outright; readers of older manifests (`scripts/summarize-test-runs.py`
  and the summary module) tolerate its absence and ignore it where present, since retained runs from before this plan
  still carry it.
- Where a test needing tmux skips for lack of one, its skip message names tmux so the recorder output shows the missing
  substrate, matching how systemd- and SSH-dependent tests report.

## Outline

Bookmarks are `plan/tmux-drop-from-tooling/<nn>-<name>`. Split further if a step is too large to review.

1. `01-no-pinned-tmux`: criteria 1, 2 and 3 (and 4's script parts) in one PR. They cannot be split: the recorder's
   `--tmux required` mode checks the pinned build that criterion 1 removes, so removing either without the other leaves
   the release gate and every documented test command refusing to run. `ci:` or `test:`.
2. `02-brick-test`: criterion 5. `test:` or `chore:`.
3. `03-instructions`: criterion 6 and the remaining test-crate and fixture parts of 4. `docs:` or `chore:`.

None of these has a user-visible effect; no changelog fragment unless a type calls for one.

### Validation

Per root `AGENTS.md`, "Tests and validation", and `.agents/validation.md`: `python3 scripts/test-record-test-run.py` and
`python3 scripts/test-test-run-traces.py` for the recorder, the hunt scripts' own tests, `deflake/` self-tests if any,
`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` through the recorder for the brick-test
harness, `dist generate --check` after a workflow change, `python3 releasing/rehearse-release-gate.py`'s self-test if it
has one, `shellcheck` on changed shell scripts, the startup shutdown's Rust tests and the stock-tmux test through the
recorder once with a machine tmux present (and once without, to see the skip), `deno test` and `deno check` for the
Tensorlake harness, `python3 scripts/check-agents-md.py` and its `--self-test` if the script changes, and
`dprint check`. Report checks run, reused and skipped.

## Fallbacks

Examples of departures that need the scope reassessment: keeping a generic "substrate identity" field in the recorder,
keeping a pinned build "for later", or rewriting frozen material.

If something tmux-shaped turns out to be genuinely needed, keep it, state why, and list it in the report's "things you
should know". Do not block for it unless keeping it contradicts a decision above.

Decisions to log: how the shutdown tests find a tmux and skip without one, what replaced the decoy in the stack-cleanup
test, which fake-agent modes and test-crate parts were kept and why, how older manifests are read, and every review
finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-tmux-drop-from-tooling-log.md` in the parent directory of the checkout you run in, derived as that section
says. Resolve it to an absolute path before you start. Scratch files go in the agent scratch directory that `AGENTS.md`,
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
available as a native sub-agent type, `$scode-harness-shellout` supplies the launch mechanics.

Slow tests on Tensorlake, and parallel work (the maintainer, 2026-11-12, for every tmux-* plan): when you run slow
end-to-end tests, especially the browser (Playwright) specs, use the `scode-ssh-delegate` skill and run them on
Tensorlake sandboxes in parallel, up to 10 sandboxes at once, per `.agents/tensorlake.md`. This is the maintainer's
request for Tensorlake that file asks for, and 10 is the budget. Also run independent work in parallel wherever that
causes no conflicts and loses no quality.

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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are
  `plan/tmux-drop-from-tooling/<nn>-<short-name>`, as the outline names them.
- Keep each PR reviewable. Avoid churn: code added in one PR and deleted in a later one means the stack should have been
  shaped differently. Within this run, a PR that needs correcting is restructured rather than corrected on top, and that
  applies to PRs an earlier run built too. Splitting a large outline step into several PRs is fine.
- Commit messages and PR titles use Conventional Commits, with the type reflecting the user-visible effect (root
  `AGENTS.md`), and a changelog fragment in the same commit when the type calls for one.
- Run every commit message, PR title and PR description through the `scode-commit-msg-reviewer` skill's cold read.
- `TODO.md`, `TRIAGE_OUTCOMES.md`, `TRIAGE_OUTCOMES_ARCHIVE.md`, `review_feedback_queue/` and `releasing/changelog.d/`
  are shared bookkeeping files: on a rebase conflict there, keep both sides.
- Before every push of a plan bookmark, run `queue status` and confirm the line still carries your claim id
  (`plans/.agents/executing.md`, "Executing one plan", step 9).
- Never edit `plans/`: the plan's state moves only through `scripts/plans-queue.py`, as `plans/AGENTS.md` describes.
- PRs stay drafts. Never mark one ready and never merge; landing is the monitor's job (`plans/AGENTS.md`).

### Review gate

Before finishing each PR, launch a review of its changes and address what it finds before moving on. The user demands
exactly this reviewer, and no review swarm: a single fresh-context agent on Opus 5.5 (`claude-opus-5-5`) at high effort,
launched natively when your harness serves that model, otherwise through `$scode-harness-shellout`. If it cannot be
reached when a review is due and is still unreachable after a reasonable retry, block the plan per
`plans/.agents/executing.md`, "Executing one plan" (step 10), after pushing what is consistent. Never substitute another
reviewer.

The prompt carries this full charter, because the reviewer has nothing else:

> Review the changes in this PR adversarially for general correctness (bugs, unhandled cases, broken invariants, wrong
> behavior against the acceptance criteria quoted below), design (whether the shape of the change fits the codebase and
> the goal, and whether a simpler shape would), and idiomatic code for the languages involved. Report findings to the
> named file, most severe first, each with the file, the quoted code, what is wrong, and the smallest fix. Edit nothing
> and touch no VCS state.

The prompt must also name the repository root (the executing checkout), the bookmark or commit range of this one PR, a
findings file in the scratch directory, and the acceptance criteria (this file's goal, acceptance criteria, Decisions
already made and outline). When a PR changes tests or their fixtures, include the full text of
`.agents/test-authoring.md` verbatim in the prompt, as root `AGENTS.md` requires. For a mostly-deleting PR, also ask the
reviewer to look for what the deletion left behind: dangling references, comments and docs describing removed behavior,
and code whose only reason was the removed path.

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
passed its review gate, and the PRs make the TODO.md and review-queue changes the decisions name. Open, not merged. If a
`## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
