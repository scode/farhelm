# Deflake runs: requirements

NOTE: This is a requirements document for the `deflake/` tooling and the agent behavior around it, written so the intent
survives the people and sessions that built it. It is not a description of the current implementation; that is
`deflake/bin/deflake` and `deflake/AGENTS.md`. When the two disagree, this file says what was wanted.

## What a deflake run is

A deflake run is one complete pass of the project's full local test suite, end to end, with the goal of DISCOVERING
flaky tests. It is not a gate and not a way to validate a change: it runs against a fixed base commit (main by default),
and it exists to find tests that fail nondeterministically on this machine so they can be recorded and later fixed. The
run is over when every battery has executed once and every failure it produced has been either recorded as a flake or
handed a fix. "Repeat until I say stop" means: when a run ends, start another one against the same base, and keep
excluding what earlier runs found.

## Requirements

**The agent must be cheap.** Inference is spent only on decisions: is this failure a flake or a deterministic
regression, and what goes in the record. Everything mechanical is done by deterministic scripts. In particular the agent
is hard barred from polling: it must not take turns to check whether tests are still running. It takes a turn when the
scripts hand it an event (a failure needing a decision, a stalled phase, a finished run, a dead daemon) or when a hard
wait limit passes and its harness returns control. This has to work without assuming a particular harness (Claude Code,
Codex, opencode, muse), so the primitive is a blocking command that returns on an event or a limit, and the harness
decides whether to run that in the background with a completion notification or in the foreground with its largest
timeout.

**The sweep continues; it never restarts.** A failure does not stop the sweep, and handling one does not restart it.
Coverage should be even: every test gets its one execution per run, and a failure early in the run costs nothing for the
tests after it. The batteries already run fail-fast off, so a phase collects all its failures and the sweep moves to the
next phase.

**Known flakes are not re-run.** Within a run, a test that flaked is not executed again except for the classification
reruns. Across runs, tests already recorded as flakes in `TODO.md` are excluded from every run, so each run spends its
time on discovery. The machine-readable form of that list is `deflake/known-flakes.txt`, one test per line, and it is
kept in lockstep with `TODO.md`: a flake is recorded in both in the same PR, and when the `TODO.md` entry is removed
(because the flake was fixed or dismissed) the `known-flakes.txt` line is removed in the same PR. A stale line there is
a test that silently never runs again, which is the opposite of what the sweep is for.

**Failures are classified mechanically before the agent sees them.** Each failed test is rerun alone a fixed number of
times, on the same build and substrate, with the recorder retaining every attempt. Failing every time is the signal for
a deterministic failure; passing even once is the signal for a flake. The agent gets the verdict, the rerun results, the
retained evidence paths, and a bounded excerpt of the failure output, and decides whether to overrule. A battery with no
per-test selection is rerun whole. A per-test battery that fails without naming a test (a build error, a refused or
timed-out runner) is NOT rerun to classify it: that would cost hours and produce a verdict out of timeouts, so it is
reported unclassified and the agent reads the log. Classification cannot see order dependence or shared-state leakage
between tests, since a rerun runs the test alone; a flake verdict says the test is nondeterministic under this machine's
conditions, not what makes it so.

**Every flake becomes a PR; every clear deterministic failure with an easy fix becomes a PR.** PRs form one linear stack
built with the jjstack workflow. They are never opened for review or merged by the agent; the maintainer does that. A
flake PR adds the `TODO.md` Deflake entry, the `FLAKES.md` dated entry with `Class:` and `Cause:` lines, and the
`known-flakes.txt` line. A deterministic failure gets a fix PR only when the cause is clear from the failure and the fix
is small and low-risk; anything that needs a design decision, is a lot of work, or carries risk is recorded in `TODO.md`
instead (as a bug, not a flake) and the sweep continues.

**The sweep runs isolated from the agent's checkout.** The agent writes PRs while the sweep is running, so the sweep
cannot share the agent's working copy: it runs in a detached git worktree of the agent's checkout, pinned at the base
commit, with its own build outputs. (A jj workspace was the first attempt; on a colocated repo it carries no `.git`, and
the recorder finds the checkout through git, so it refused every recorded phase.) This also keeps the project's
one-agent-per-checkout rule intact: the worktree is keyed by the checkout that owns it, and a worktree created by a
different checkout is refused rather than reused. The known-flakes list is the one thing read live from the agent's
checkout, so a flake recorded mid-run is excluded from the next phase and the next run; a list that cannot be read is
reported as an event, never silently treated as empty.

**One sweep per machine.** The batteries assume a quiet machine, so two sweeps at once would find load flakes, not test
flakes. The driver holds one lock under its state root and refuses a second `start` while a daemon lives. That fixed
lock path, and the fixed worktree path per checkout, are the deliberate exception the repository's fixed-path rule
allows for locks; every port, temporary directory, and unit name inside the batteries still comes from the harnesses'
own per-run choices.

**Evidence is retained the project's way.** Every battery and every classification rerun goes through
`scripts/record-test-run.py`, so a flake record can cite a run id and a substrate identity, and a later pass never
overwrites a failure.

**A wedged phase is reported, not hidden.** A phase whose output stops growing for a long time produces a stall event
for the agent to look at, and a hard per-phase timeout eventually ends it and reports the phase as failed. A daemon that
dies produces an event too. Silence is never the signal that everything is fine; the finished run is its own event.

## Scope

The sweep is the AGENTS.md finishing-work inventory minus what cannot flake: formatters, clippy, compile-only checks,
generated-file and asset-parity checks, and shellcheck are not run. `scripts/test-tmux-pinned-shutdown.sh` is not run
separately because the workspace nextest phase executes the same tests on the same pinned tmux. Slow deterministic
batteries (CentOS provisioning, desktop smoke) ARE run; the run is meant to take hours in the background and their cost
in inference is zero when green.

The browser suite runs both engines, as the merge gate does.

## Non-goals

This tooling does not decide what a flake's cause is, does not fix flakes on its own, does not validate PRs (the
finishing-work gates still apply to each PR the agent makes), and is not a replacement for the narrow-test ladder in
`.agents/narrow-tests.md` when a specific failure is being investigated. It also does not manage the live install on
this machine; it creates its own supervisors and helms from temporary state like every other test harness here.
