# Require ownership of test state parents

## What this was about

Farhelm's test recorder and deflake sweep accepted a sticky parent directory owned by another account. That account
could still replace entries, contrary to the tools' documented ownership protection. The triage decision was to enforce
ownership in both tools. Each now requires its state directory's immediate parent to belong to the account running the
tool or to root.

## Things you should know

The normal root-owned system temporary directory still qualifies. A custom recorder output location is subject to the
same immediate-parent check. The triage outcome moved to the archive with the draft PR link.

The evaluation delegate read part of the validation inventory before its output was truncated. Its prescribed evaluation
steps completed, and I checked the actual event stream and cleanup state.

## Open questions and possible follow-ups

None.

## Pull requests

- Require ownership in the test recorder and deflake, with tests and docs.
  [#4386](https://github.com/scode/farhelm/pull/4386/changes)

## Checks

- On the working tree captured in `9badcd35`, `python3 -B scripts/test-record-test-run.py` passed 58 (recorder
  `8b4b9a73-af40-4ccc-a83a-9157b5d5a4c1`). `python3 -B deflake/test-deflake.py` passed 16 (recorder
  `4989e2fd-67b5-429a-9946-701eedc53a0b`). After tightening the fixtures, the recorder ownership case passed on its own
  (recorder `17a6f125-705f-4284-a210-09b1de2c82d4`) and all three deflake state-root cases passed (recorder
  `0b585c42-50c9-430e-8513-ccafcd686c78`). Those two attempts cover the corrected test files captured in `2ac3e5b4`. The
  final rebase kept the checked driver, recorder and corrected test files byte-identical.
- Ran `deflake/bin/deflake start --profile eval` and the background `wait` loop through the cold delegate in the
  isolated EVAL checkout. Run `611ed519-74b8-47a9-9ae9-1093c36eb092` on fixture revision `17f3e782d90f` classified the
  planted Rust flake after three passing reruns (initial recorder `7aceb28d-5d82-4b39-88a0-aa5d4fefc141`) and the
  planted JS failure after three failing reruns (initial recorder `b5335197-87e0-448a-b475-6b0fe2656161`). The delegate
  made local flake and fix commits. After the finish event, `wait` returned exit 4, confirming the run was over with no
  pending events. I verified build-output cleanup.
- Targeted `dprint check`, `git diff --check`, instruction-link and changelog format checks passed. The instruction
  checker still reports the existing archived heading warning. The focused Python checks and isolated sweep cover the
  changed tooling, so a full runtime workspace sweep was skipped.

## Review

The independent source review and commit wording read passed. The review led to a simulated non-root effective UID in
both fixtures, so the root-owned parent case detects a regression when the tests run as root.

### Landing

#### What landed with it

Nothing else landed in this round. Since this plan's work started, main gained only three test-only changes, to the
release brick test and the supervisor's installer code, none of which touches the test recorder or the deflake sweep.

#### Fixes made while landing

One went in as a second PR, [#4389](https://github.com/scode/farhelm/pull/4389/changes). The plan added the test
recorder's own tests to the "run all tests and fix" list, where they were already listed further down, so following the
list would have run them twice. The new entry now names only the deflake sweep's tests.

#### Checks

The plan changes only developer tooling: the test recorder and the deflake sweep, their own tests, their docs and two
agent instruction files. No product code changed, so the landing ran those tools' own test suites and the instruction
and formatting checks, all locally on x86_64:

- `python3 -B scripts/test-record-test-run.py` (the recorder's own tests), through the recorder (recorder run
  `19da6dde-3b1d-4b26-9837-176f74483960`): all 58 passed.
- `python3 -B deflake/test-deflake.py` (the deflake sweep's own tests), through the recorder (recorder run
  `87706b5c-6540-4f79-8e40-1a04dbbd6406`): all 16 passed.
- `dprint check`, `python3 scripts/check-agents-md.py` and `python3 releasing/check-changelog.py format` (formatting,
  instruction and changelog lints): passed.

The landing fix (#4389) changes one line of agent instructions: `dprint check` and `python3 scripts/check-agents-md.py`
passed again. Reused: the executor's end-to-end evaluation run of the deflake sweep (in its report), since neither the
rebase nor the landing fix touched the sweep. Skipped: every Rust, desktop app and smoke run, since no product code
changed.

#### Things you should know that the report does not say

An independent reviewer and I checked that the stricter rule refuses none of the directory layouts the project's own
tools, test harnesses and CI use. The only parent it newly refuses is one marked sticky that belongs to another non-root
account. The tools create their default parent themselves, so it belongs to the running user. The system temporary
directory belongs to root, on macOS too. The test worker, boat and Tensorlake harnesses each run as a single account,
and CI and release jobs write under the runner's own temporary directory.

#### What you should still do

Nothing.
