# Pin the Rust toolchain

Written against main at e7b47815 on 2026-11-08. This is a goal file for an unattended run, executed through the planning
system in `plans/AGENTS.md`. You, the executing agent, have none of the conversation that produced it; everything you
need is here or in the repository files it names. SPEC.md and SPEC_impl.md stay authoritative over anything this file
says.

## The goal

The TODO.md entry (bucket "Near term"), verbatim:

> **Pin the Rust toolchain.** Nothing pins Rust today: there is no `rust-toolchain.toml`, CI's toolchain action installs
> whatever stable is current when the job runs, and a developer machine keeps whatever stable it last updated to. So
> local checks and CI can lint with different clippies without anyone choosing that. On 2026-10-09 CI was on 1.99 and
> the local machine on 1.98.1, and CI's clippy failed on lints the local one did not have yet (fixed in #1742). Pin an
> exact version so local, CI and the release builds agree, and make toolchain bumps deliberate PRs. First check that the
> cargo-dist release build jobs honor the pin: they install Rust only when it is missing and otherwise use the runner's
> own. Once pinned, drop the newest-stable clippy step from `.agents/run-all-tests.md`.

What is true today, from a read of main at e7b47815 (line numbers approximate):

- No `rust-toolchain.toml` or `rust-toolchain` file exists.
- `.github/workflows/ci.yml` uses `actions-rust-lang/setup-rust-toolchain` (pinned by commit, v2.0.0) in several jobs
  (around 66, 207, 245, 292, 346, 489) with no explicit toolchain input.
- `.github/dist-build-setup.yml` around 59-62 says deliberately: no `setup-rust-toolchain` in the release path; dist's
  build jobs use the runner image's preinstalled stable and add targets with rustup. It adds `wasm32-unknown-unknown`
  itself (around 383-385) and has a macOS `rust-objcopy` workaround tied to Rust 1.98.0 (around 413-431).
  `.github/workflows/release.yml` is generated from `dist-workspace.toml`; never hand-edit it (`releasing/AGENTS.md`).
- `scripts/build-tmux-assets.sh` provisions its own (zig) toolchain for tmux; it is not a Rust toolchain question.
- `.agents/run-all-tests.md` around 79-89 has the newest-stable clippy item to drop; line 12 (the 1.99 versus 1.98.1
  story) and line 79 (the list naming "the newest-stable clippy") mention it too. `.agents/validation.md` around 534
  uses a nightly toolchain for coverage (`llvm-tools-preview`), which must keep working beside a pin.
- Other scripts that call rustup or cargo with a toolchain: `scripts/devbuild-mac.sh`,
  `scripts/test-provision-centos.sh`, `scripts/readme-screenshot.sh`, `scripts/docs-screenshots.sh`,
  `scripts/readme-video.sh`.
- The current stable on 2026-11-08 is 1.99.1.

### Acceptance criteria

- A `rust-toolchain.toml` at the checkout root pins an exact version: the newest stable when this plan runs (look it up
  then; 1.99.1 at planning time), with only the components and targets every build needs (likely clippy, rustfmt and
  `wasm32-unknown-unknown` for the UI). Per-purpose targets (devbuild's musl cross targets, the CentOS provisioning
  triple) stay out: those scripts keep adding them with `rustup target add`, which then applies to the pinned toolchain.
- Check whether the macOS `rust-objcopy` workaround in `dist-build-setup.yml` (tied to Rust 1.98.0) is still needed or
  still right under the pin; log what you find, and rework it only if it is wrong.
- CI uses the pinned version in every Rust job. Verify how the pinned toolchain action treats the file; if it needs an
  explicit input, give it one derived from the file, not a second copy of the version.
- The release build jobs use the pinned version. Check by reading the generated workflow, `dist-build-setup.yml` and how
  rustup on the runner handles a toolchain file (whether it installs the pinned toolchain on first use with the runner's
  rustup version). Fix what does not honor the pin through `dist-workspace.toml` or `.github/dist-build-setup.yml`
  (never by hand-editing `release.yml`), keeping the "no second toolchain manager" reasoning in mind. If you cannot
  prove from those sources that the release builds honor the pin, ship anyway and say so plainly in the report: the next
  dev release confirms it.
- Repository scripts that pick a toolchain keep working with the pin, and the nightly coverage path still works.
- The newest-stable clippy item is dropped from `.agents/run-all-tests.md`, and the inventory in `.agents/validation.md`
  and anything else naming the floating toolchain is updated (root `AGENTS.md`, "Tests and validation", requires the
  inventory and the run-all-tests lists to change with the workflows).
- Bumping is a manual PR changing the one version; say so where developer setup or the release docs describe the
  toolchain, briefly.
- Typed `chore` (no user-visible effect): no changelog fragment unless you find one (root `AGENTS.md`, "Changelog
  fragments").
- The last PR removes the TODO.md entry quoted above.

## Decisions already made

Settled with the maintainer on 2026-11-08:

- **Version: the newest stable when the plan runs.** The alternative was exactly 1.99.1.
- **Bumps are manual PRs** when someone wants a newer Rust. The alternative was having the run-all-tests procedure
  report a newer stable.
- **If the release builds cannot be proven to honor the pin without cutting a release, ship it**, state the gap in the
  report, and let the next dev release confirm it. The alternative was blocking. A plan never cuts a release tag.
- **Reviewer:** a single fresh-context agent on gpt-6.1-sol at high effort (see Review gate). If it cannot be reached
  when a review is due and is still unreachable after a reasonable retry, block the plan; no substitute reviewer.
- **No-workhorse mode**, the plans default, stated on the planning page.
- **Planned on hold**, at the maintainer's request, so it runs only once they unhold it.

## Requirement sources

**The user's request:** the TODO.md entry and the decisions above.

**Binding repository constraints:** `releasing/AGENTS.md` (read it before touching `dist-workspace.toml`,
`.github/dist-build-setup.yml` or anything under `releasing/`; `release.yml` is generated); `.agents/validation.md` and
`.agents/run-all-tests.md` (inventory and lists change with the workflows); root `AGENTS.md`, "Tests and validation",
"Conventional Commits", "Changelog fragments", "TODO.md and FLAKES.md", "Instructions by path".

**Planner proposals:** a single `rust-toolchain.toml` as the one place the version lives.

## Outline

Likely one PR, `plan/pin-rust-toolchain/01-pin-toolchain`, typed `chore`: the toolchain file, any CI and release-path
adjustments, the script and documentation updates, and the TODO.md removal. Split off the release-path change as its own
PR if it is more than a line or two, so it can be reviewed on its own.

### Validation

Build and clippy the workspace with the pinned toolchain (`cargo clippy --all-targets -- -D warnings` and
`cargo clippy -p farhelm --bins -- -D warnings`), and fix any lints the pinned version raises in the same PR. Run
`cargo fmt --check`. If the workflows change, check them with whatever workflow linting `.agents/validation.md` names,
and consider one on-demand CI dispatch on the PR (the CI workflow runs only on demand; dispatching it is allowed,
marking the PR ready is not). Do not run the full test battery for a toolchain pin.

## Fallbacks

Examples of departures that need the scope reassessment: adding a toolchain installer step to the release path, adding
automation for bumps, or changing the dist version.

Decisions to log: the exact version and components, how CI and the release path read the pin, what was verified about
the release builds versus inferred, and every review finding you decided not to follow.

## How to run

### Files outside the repository

Per `plans/.agents/outside-files.md`, "Files outside the repository": this plan's working log is
`farhelm-plan-pin-rust-toolchain-log.md` in the parent directory of the checkout you run in, derived as that section
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
  "Executing one plan" (step 6); never another plan's PRs. Bookmarks are `plan/pin-rust-toolchain/<nn>-<short-name>`, as
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
passed its review gate, and the last code PR makes the TODO.md changes the acceptance criteria name. Open, not merged.
If a `## Decisions` section exists, its latest entry must also be satisfied. Then close the plan per
`plans/.agents/executing.md`, "Executing one plan" (steps 11 and 12): deliver its report through the queue script, write
a closing entry in its log, and stop the watchdog. Never edit `plans/` yourself.
