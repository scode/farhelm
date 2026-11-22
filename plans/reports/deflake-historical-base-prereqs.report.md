## What this was about

A test sweep against an old revision could look clean while missing tests or using a different tmux setup than that
revision expected. Deflake now requires the selected revision to contain `1eb4e37517d2164d2bd35753dfcbad3f03545451`, the
last change to the test list and preparation steps. It prints the floor and its reason before creating any run state
when a base is refused.

## Things you should know

- A branch that forked before the floor also needs that commit in its history. Deflake pins a supported base before
  starting the daemon and checks it again before preparing the worktree, so a branch move cannot change the sweep after
  admission.
- The first evaluation completed the sweep and flagged uncertainty about whether to read the full validation inventory
  before correcting the planted assertion. The evaluation now names the test-authoring checklist as the reading for that
  fixture-only fix.
- Raise the compatibility floor when new test batteries, preparation steps, or recorder behavior make older bases
  incompatible.

## Open questions and possible follow-ups

None.

## Pull requests

- Reject unsupported revisions before starting deflake. [#4550](https://github.com/scode/farhelm/pull/4550/changes)

## Checks

- `python3 -B deflake/test-deflake.py` passed 20 tests at draft head `758f9ebb768e`, recorder
  `495b2f30-2d62-45ff-b640-f0e25ad7c1cf`. The ancestry cases include the floor itself, a descendant, the floor's parent,
  a side branch, an unknown base and an unavailable floor.
- In the isolated evaluation checkout,
  `deflake/bin/deflake start --profile eval --base 1c097e7b82f123c26c85eb0de90096f0ae22d093` exited 1 with the ancestry
  diagnostic. `deflake/bin/deflake status` found no current run, and no run metadata was created for that checkout.
  Reused this check from `62016f5c6286` because the entire driver, including CLI parsing and run initialization, is
  byte-identical at the final head.
- The revised `deflake/EVAL.md` procedure ran at `9df4e65f1941`, the final draft plus throwaway fixtures, run
  `bf459a2c-444f-4ebb-9e10-a3efe7a39c7b`. `deflake/bin/deflake start --profile eval` used system tmux 3.4, recorded in
  the run's `phases/prepare-tmux.log`. Deflake classified the planted Rust failure as flaky after three successful
  reruns (recorder `a3087fc9-fef5-4747-9bcb-283deb1bfdff`) and the JS assertion as deterministic after three failures
  (`59cf5e6e-c6a4-4480-9653-09081cc4c195`). The evaluator committed the flake records and corrected the planted
  assertion in a second commit. The final `deflake/bin/deflake wait` returned 4, meaning the sweep finished with every
  event acknowledged. The sweep removed its build output. The evaluator independently ran the corrected assertion
  outside the requested procedure. It was unrecorded and contributes no evidence to the verdict.
- `dprint check` on edited Markdown, `git diff --check`, and `python3 scripts/check-agents-md.py` passed. The
  instruction checker reported four existing citation warnings. The full workspace battery was skipped because the
  change is confined to the deflake tooling and its documentation. Upstream supervisor and desktop changes since the
  draft's base leave the driver, recorder and sweep configuration unchanged.

## Review gate

The independent high-effort source review found three issues. The test fixtures now disable inherited Git signing, all
base-resolution failures explain the floor, and the CLI help describes ancestry. The executor verified the corrections
in the final diff and repeated the unit and evaluation checks. The source gate is satisfied. The cold wording check
confirmed that the draft's title and description communicate the compatibility rule.
