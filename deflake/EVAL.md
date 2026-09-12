# Evaluating the deflake tooling

NOTE: This is the procedure for checking that the deflake driver and `deflake/AGENTS.md` still work end to end after a
change to either. It is not a deflake run and finds no real flakes. Use it when you change `deflake/bin/deflake`, the
agent instructions, or the recorder's interfaces the driver depends on.

## What it proves

A separate, low-power agent that has never seen this tooling is given only "start a deflake run" and the checked-in
instructions, and must: start the driver, block on `wait` without polling, record a planted flake in `TODO.md`,
`FLAKES.md` and `deflake/known-flakes.txt`, fix a planted wrong assertion as its own commit, and reach `wait` exit 4. It
was first run on 2026-09-12 with a Sonnet-class model; the first two passes each found a real bug in the driver, the
third passed. That history is why the agent's report format below asks for every event verbatim: the report is the
evidence, and a low-power model's confusions are exactly the instruction gaps the eval exists to catch.

## Procedure

1. From your checkout, with the driver and instructions you want to test committed on the current working-copy parent,
   run `deflake/eval-setup.sh <empty dir>`. It prints the eval checkout path. It clones your checkout, points `origin`
   at a local bare repository so nothing reaches GitHub, initializes jj, and commits two fixtures on that clone's
   `main`: a farhelm-proto unit test that fails once per marker file and a JS test with a wrong assertion.
2. Make sure no sweep is running on the machine (`deflake/bin/deflake status` in your checkout); the daemon lock allows
   one at a time.
3. Delegate the run to a fresh, low-power agent (a Sonnet-class model at medium effort is the intended calibration: the
   instructions have to be unambiguous for it, not just for you), with the prompt below. Do not coach it further.
4. Read its report against the expected outcome. Anything it found ambiguous is an instruction bug; anything the driver
   did that the report describes as unexpected is a driver bug. Fix, re-run `eval-setup.sh` into a fresh directory (the
   old clone carries the old driver), and repeat until a pass is clean.
5. Clean up: delete the eval directory, the sweep worktree the driver made for it under
   `~/.local/state/farhelm-deflake/workspaces/<key>` (its `status.json` under `runs/<uuid>/` names the path), the
   pointer under `current/`, and `/tmp/deflake-eval-flaky-marker`.

The first pass pays a cold build in the eval worktree, about ten minutes on a warm cache; later passes into a new
directory pay it again because the worktree is keyed by checkout path. The sweep itself takes a few minutes.

## Delegate prompt

Replace `<eval checkout>` with the path the setup script printed. The deviations are the only differences from a real
run: no GitHub, no jjstack, no gates beyond `dprint check`, and the eval profile.

```
You are an agent working in the repository checkout at <eval checkout> (a jj + git colocated checkout; treat it as
the project checkout for everything below, and run all commands with that directory as cwd). The user's request to
you is:

"start a deflake run"

Read that checkout's AGENTS.md section "Deflake runs", then deflake/SPEC.md and deflake/AGENTS.md, and follow
deflake/AGENTS.md exactly, with ONLY these deviations because this is an evaluation of the tooling:

1. Start the driver with `deflake/bin/deflake start --profile eval`. Everything else about the protocol is unchanged.
2. For waiting: run `deflake/bin/deflake wait` in the background with your harness's largest tool timeout and do
   nothing until its completion notification arrives. Do not poll, tail logs, read its output file, or run `status`
   between events. If `wait` exits 3, run it again the same way.
3. PRs: the jjstack skill and GitHub are NOT available. Where deflake/AGENTS.md says to make a PR, make the commit with
   `jj commit -m ...` (one commit per record or fix, stacked linearly, Conventional Commit format) and set a bookmark
   `pr/<slug>` on it. Do not push, do not run any `gh` command. Run no finishing-work gate except `dprint check` on
   files you edited.
4. Do not modify anything under deflake/ except deflake/known-flakes.txt.
5. Never touch any other checkout or anything under ~/.local/state/farhelm. The driver's own state under
   ~/.local/state/farhelm-deflake is fine.

Keep going until `wait` returns exit 4, or until deflake/AGENTS.md tells you to end the run on a tooling event. Then
write your final report, containing in this order: every event you received, verbatim as `wait` printed it; for each
event, what you decided and did, the exact commit message and bookmark of any commit, and the full text you added to
TODO.md, FLAKES.md and deflake/known-flakes.txt; the output of
`jj log -r 'main..@' --no-graph -T 'change_id.short() ++ " " ++ bookmarks ++ " " ++ description.first_line() ++ "\n"'`;
how many times you called `wait` and how many returned 3; any instruction in deflake/AGENTS.md or deflake/SPEC.md you
found ambiguous, contradictory, or impossible to follow, quoted, with what you did instead; and anything the tooling
did that did not match its documentation.
```

## Expected outcome of a clean pass

Three events, in this order, then exit 4:

- `failure` on `nextest farhelm-proto deflake_eval::tests::deflake_eval_flaky_fails_once_then_passes`, verdict `flake`,
  reruns `pass, pass, pass`, an excerpt starting at the test's `FAIL [` line and containing the panic message. The agent
  makes one commit touching `TODO.md`, `FLAKES.md` (with `Class:` and `Cause:` lines) and `deflake/known-flakes.txt` (a
  line matching the event's `test` field, then two spaces and a `# slug` comment).
- `failure` on `phase js-unit`, verdict `deterministic`, reruns `exit 1` three times, an excerpt showing the `41 !== 42`
  assertion. The agent makes one commit correcting the assertion (a wrong assertion is the instructions' own example of
  an easy fix); recording it in `TODO.md` instead is also acceptable, filing nothing is not.
- `sweep-finished` listing the one flake found.

Zero `wait` calls returning 3 is normal for this profile; the events are queued before the agent's wait limit. A report
that mentions a `daemon-error`, `phase-error` or `daemon-died` event is a failed pass whatever else happened.
