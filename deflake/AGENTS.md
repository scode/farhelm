# Deflake run: agent instructions

Read `deflake/SPEC.md` first; it says what this is for and what the rules below protect. This file is the protocol.
"Start a deflake run" and "start a deflake run and repeat until I say stop" both mean exactly this procedure.

## Before starting

- Confirm the checkout has no active jjstack (an empty working copy on main, or a stack the user has said to build on).
  The flake PRs go on a linear stack; if one already exists, they go on top of it, and say so.
- Fetch and make sure local `main` is at origin's main. The sweep runs against `main` unless the user names another
  base.
- Do NOT prebuild anything, run gates, or read test files. The driver prepares its own worktree.
- Only one sweep can run on the machine at a time. If `start` reports that another daemon holds the lock, another
  agent's sweep is running; tell the user and stop rather than waiting for it or stopping it.

## The loop

Start the driver from the checkout root. Add `--repeat` only when the user asked for repeat mode.

```
deflake/bin/deflake start
deflake/bin/deflake start --repeat
```

Then block on `deflake/bin/deflake wait`. This is the entire waiting protocol, and it is where the inference budget is
saved: you do not check status, tail logs, look at the run directory, read the background command's output file, or
otherwise spend a turn until `wait` returns. How to block depends on the harness:

- If the harness can run a command in the background and notify you when it exits (Claude Code's `run_in_background`),
  run `wait` that way and do nothing until the notification arrives.
- Otherwise run `wait` in the foreground with the largest tool timeout the harness allows.

Either way, set the harness's tool timeout to its maximum and leave `--max-wait` at its default of 540 seconds, which
fits under a 600 second tool timeout with room for the process to exit cleanly; a longer `--max-wait` risks the harness
killing `wait` before it can print, which leaves you with no exit code to act on. When `wait` returns 3 (limit passed,
nothing new), call it again immediately and without comment; that one call per limit period is the only permitted
polling. Do not run `start` and `wait` in the same shell command line: `start` returns at once and `wait` is a separate
call.

`wait` exits 0 with an event printed, 3 when the limit passed with nothing to do, 4 when the run is over and every event
is acknowledged, and 1 when there is no current run for this checkout (you have not started one, or you are in the wrong
checkout). `status` and `events` exist for when the user asks, not for the loop.

## Handling an event

Every event is answered with `deflake/bin/deflake ack <id>` once handled, then `wait` again. Events:

**`failure` with verdict `flake`.** The test failed in the sweep and passed at least once in the classification reruns.
Do a light investigation only: read the excerpt and, if needed, the retained evidence directory it names. Do not climb
the narrow-test ladder, do not repeat runs, do not look for a fix. Then make one PR that records it:

- A `TODO.md` entry under `## Deflake`, in the file's existing style, naming the test and its file, what was observed,
  the run ids of the sweep failure and the reruns, and a one-line hypothesis if the excerpt supports one. If `TODO.md`
  already has an entry for this test (search it by test name first), do not add a second one: append this run's evidence
  to the existing entry in one sentence instead.
- A dated `FLAKES.md` entry per that file's header: observation paragraph, evidence fields, `Class:` and `Cause:` lines.
  A guessed cause is `Cause: hypothesis`; no idea is `Cause: unknown`.
- The `deflake/known-flakes.txt` line, in the exact form the event's `test` field prints, followed by two spaces and a
  `# <slug>` comment where the slug is a few kebab-case words a reader can use to find the TODO entry (the entries have
  no formal handles). An existing line for the test means the exclusion was not in effect when the sweep started; keep
  the line and say so in the record.

**`failure` with verdict `deterministic`.** Failed every rerun. Decide quickly from the excerpt whether the cause is
clear AND the fix is small and low-risk (a wrong assertion, a missing prerequisite, a renamed symbol). If so, make the
fix PR with the usual per-PR gates for that change. If not (needs a design decision, is a lot of work, or carries risk),
record it in `TODO.md` under the bucket that fits (a product bug goes in `## Tricky bugs`, not Deflake) with the
evidence paths, and continue. Either way do not stop the sweep and do not rerun the sweep.

**`failure` with verdict `unclassified`.** No reruns were completed. For a per-test id that means the daemon was
stopping; treat it as a flake report with `Cause: unknown` and say in the record that classification did not happen. For
a `phase` id on a nextest or Playwright phase it means the battery failed without naming a test (build error, recorder
refusal, runner timeout), and the driver deliberately does not rerun a whole battery to classify that: read the phase
log's tail, and treat it as an environment or tooling problem to report to the user, not as a flake to record.

**`failure` on a `phase` id of a monolithic battery** (doctests, install.sh, CentOS, desktop smoke, JS). The whole
battery was rerun three times, so the verdict means what it does for a test, and the flake and deterministic rules above
apply unchanged: when the excerpt names the failing test or scenario, record or fix that. Two extra outcomes are
specific to whole batteries. A deterministic failure is often an environment problem (a missing package, a tool below
its floor); when it is, tell the user what is missing rather than filing a PR. And a battery that cannot run on this
host at all can be kept out of every sweep with a `phase <name>` line in `deflake/known-flakes.txt`, paired with a
TODO.md entry like any other exclusion.

**`stalled`.** No output for the stall window. Look at the named log's tail once. If it is a legitimately long step (a
cold cargo build, the CentOS musl build, a 20 minute e2e module), ack and wait. If it is wedged, `stop` the run, tell
the user, and end.

**`sweep-finished`.** One run is complete. Report the flakes found (the event lists them) and the PRs made. In repeat
mode the next sweep is already running; ack and wait. Otherwise ack, and `wait` will return 4: the deflake run is over.

**`prepare-failed`, `daemon-error`, `daemon-died`, `known-flakes-unreadable`, `phase-error`.** These are tooling
problems, not test results. Read the named log, ack the event, and if `status` says the daemon is still alive run
`deflake/bin/deflake stop`. Then end the deflake run: tell the user what happened, with the log's relevant lines quoted,
and do not call `wait` again. A `phase-error` is the one exception: the daemon survives it and moves to the next phase,
so ack it, report it in your final summary, and keep waiting.

## PRs

Use the jjstack skill. One PR per recorded flake and one per deterministic fix, on one linear stack, in the order the
events arrived. Never open a PR for review or merge one; the maintainer does that. Commit messages follow the repo's
Conventional Commit rules: `docs: record <test> as a flake` for a record, `fix:`/`test:` for a fix. Run the finishing
gates that cover each PR's own diff; a record-only PR needs only `dprint check`.

## Stopping

"Stop" from the user means `deflake/bin/deflake stop`, then a final report: sweeps completed, flakes found, PRs made,
the stack's bookmarks, and anything unacknowledged.

## Keeping known-flakes.txt honest

Outside deflake runs, this applies to every agent touching `TODO.md`: when a Deflake entry is removed because the flake
was fixed or dismissed, remove its `deflake/known-flakes.txt` line in the same PR. The file is read by the sweep as an
exclusion list, so a stale line is a test that never runs again.
