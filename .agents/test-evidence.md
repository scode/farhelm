# Test evidence: the recorder, retained failures and flake records

The root `AGENTS.md` sections "Reproducing failures: narrow tests first" and "FLAKES.md", moved here unchanged: running
tests through the recorder, keeping failed runs, reporting across runs, and how a FLAKES.md entry is written. Read it
before running any runtime test, and before retrying, deleting or reporting on test runs (`AGENTS.md`, "Tests and
validation"). The rules that apply to every task are also in the root, word for word; when you change one copy, change
both.

## Reproducing failures: narrow tests first

Run local and worker runtime tests through `python3 scripts/record-test-run.py`, including each reproduction attempt.
Use Python 3.13 or newer on macOS: the recorder requires `os.waitid` with `WNOWAIT` to keep child ownership through
cleanup. The release setup selects Python 3.13 explicitly on that platform. The commands in the gate inventory and
narrow-test recipe are the child argv after its `--`. Supply the actual selection and concurrency, `--kind development`
for ordinary validation or `--kind repetition` for a focused attempt, and an appropriate tmux mode. Use
`--tmux required` with the built `.ci-tmux` first on PATH for controlled comparisons; `warn` records a different
developer substrate visibly, and `none` is for checks that do not use tmux. The wrapper scrubs ambient `FARHELM_*`
before test-process startup. Retain a variable only by naming an intentional test input with `--keep-farhelm-env`;
values stay out of metadata. The recorder reserves `FARHELM_TEST_TRACE_DIR` for a fresh private directory per command,
overriding any ambient value even when retention requests it. Never fix this by changing the test process's own
environment.

Keep the failed UUID run directory before retrying, and record its path and disposition in the session's working log. A
later pass is another observation, not a replacement for the failure. Preserve failure evidence until a fix PR or
latent-flake entry records what happened; preserve same-session failures privately without adding them to FLAKES.md.
Keep paths, raw logs, and host identities out of public source. Public summaries use redacted commands, portable
substrate details, run IDs, and confidence about the cause. Record an unavailable or incomplete identity as such rather
than reconstructing it from what a machine was supposed to run. No mandatory repetition count or extra CI run follows
from this retention rule. Storage, schema, interruption limits, and examples are in `docs/test-run-evidence.md`.

When reporting across retained runs, use `python3 scripts/summarize-test-runs.py --root /path/to/retained-runs` and
archive its portable JSON where the operator chooses. Supply additional roots or individual `--run`/`--batch`
directories explicitly. Record incomplete discovery and coverage gaps alongside totals; retained failure-only release
artifacts are not a denominator for all releases. This is a manual reporting aid, not another validation gate.

The existing release gates use the same recorder. Collection follows those gates and uploads selected bounded records
when a failure has occurred by that point; later packaging failures cannot trigger it. Download useful failure artifacts
before hosted retention expires. Successful jobs do not upload run records, so those artifacts alone cannot establish a
release pass/fail denominator. Collection also exports the fixed, bounded persistent trace layout, including after a
recorder death; keep its collection summary and raw traces when recovery is incomplete. The recovery command and limits
are in `docs/test-run-evidence.md`. To validate changes to the recorder itself, use
`python3 scripts/test-record-test-run.py` and `python3 scripts/test-test-run-traces.py`, choosing affected contracts as
usual; this adds no ordinary CI test job.

The check inventory in `.agents/validation.md` is a gate, not a debugging tool. When investigating a failing, flaky, or
suspicious test — including one first seen in a full-battery run — reproduce it with the narrowest run that could show
it, and widen only when the narrow run will not reproduce: the exact test in a repetition loop, then its module or spec
file, then its test binary or engine, then the full battery. `.agents/narrow-tests.md` is the recipe, with exact
commands and caveats for every suite. Running a many-minute battery to check a one-test hypothesis is the failure mode
this rule exists to prevent.

## FLAKES.md

`FLAKES.md` is the append-only log of LATENT flakes: tests that pass where they were written and fail elsewhere,
sometimes, or under load, whose cause was not obvious when they failed. When you establish that a failure is a true
flake of that kind — pre-existing, or one that survived past the session that wrote it — add a dated entry: one
paragraph naming the test and its file (no line numbers), what was observed and where, the cause found or suspected, and
the disposition (fixed in which PR, ignored with what reason, or open). Fixed ones stay; a fixed flake that recurs gets
a new dated entry. Do NOT log a test that was written, flaked, and was fixed within the same session: that is ordinary
development, and logging it buries the tech debt the file exists to show. Open flakes also get a Deflake entry in
TODO.md and its `deflake/known-flakes.txt` line, as `deflake/AGENTS.md` describes for a deflake run's flakes; this
covers flakes recorded from now on, not older entries. The log keeps the history, TODO.md keeps the work.

For new entries, follow FLAKES.md's evidence fields and its `Class:` / `Cause:` lines. A guessed cause is a hypothesis,
even if a later run passed. Do not rewrite older entries to make them fit the new fields.
