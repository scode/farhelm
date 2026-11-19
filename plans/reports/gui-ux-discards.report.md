## What this was about

The 2026-11-18 GUI UX triage left fifty approved discards in the review queue. The maintainer chose one PR to carry out
all fifty decisions. The PR removes those entries from the queue and records each decision in the archive.

## Things you should know

Two approved discards covered the same temporary allocation while formatting a launch failure. The review found no
measured slowdown when Farhelm displayed the failure.

[#3209](https://github.com/scode/farhelm/pull/3209/changes) lowered peak memory when restoring terminal screens and
strengthened the main regression test. The remaining concern was in a separate snapshot test. That test can miss a short
memory spike if the process has already reached a higher peak. Triage found no instance of a regression passing for that
reason and approved discarding the remaining concern.

## Open questions and possible follow-ups

None.

## Pull requests

- Carry out the fifty approved discards. [#4203](https://github.com/scode/farhelm/pull/4203/changes)

## Checks

- `dprint check TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md` and Git's whitespace check
  passed.
- Compared the sorted queue index with the feedback files. All 796 surviving entries matched.
- `python3 scripts/check-agents-md.py` passed with existing citation warnings.
- Skipped runtime tests because the change is review-queue bookkeeping.

## Review gate

The independent review confirmed that only the fifty approved entries moved and every other queue entry stayed in place.
A cold read checked the commit title.

### Landing

#### What landed with it

It landed in a round of five plans: drawn-frame-actions, one-helm-per-supervisor, gui-ux-discards,
gui-ux-backend-test-gaps and gui-ux-gui-test-gaps. Since its base, main gained the previous rounds' plans (Shift+drag
marking, model family words, unusable-folder refusals and earlier ones) and bookkeeping. One discarded item asked that
the launcher act only on what the user was shown; it was discarded because drawn-frame-actions, landing in this round,
now does that.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The bookkeeping is exact: it
removes the 50 feedback items the goal lists, each archived once with its text unchanged apart from its Execution field,
and touches only the three bookkeeping files. The two entries that call an item a duplicate point at items still queued.

#### Fixes made while landing

None. Stacking five plans that each edit the triage records conflicted only in those files; I resolved them by keeping
both sides, and a reviewer confirmed nothing was lost or duplicated across the round.

#### Checks

The five plans change the desktop app's GUI crate, the supervisor, the helm, the shared protocol, a patch to the
vendored GPUI Linux backend, the terminal emulator's tests, the brick-test harness's tests, the dependency lockfile and
the specs. The landing ran the lints and the dependency checks, the whole Rust workspace suite, the desktop process's
tests, the vendored GPUI core's tests, the brick-test harness's tests, the app's end-to-end smoke and the reference
screenshots (goldens). Builds ran locally; the workspace suite and the smoke ran on boat sandboxes (cloud VMs), from
binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `python3 releasing/check-changelog.py format`,
  `python3 scripts/check-agents-md.py`, `python3 -B scripts/test-gui-e2e-smoke.py`,
  `cargo-deny --locked check licenses sources`, `python3 -B scripts/check-crate-layering.py`,
  `python3 -B scripts/dependency-notices.py --check --only-if-inputs-changed-since origin/main`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, and
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` once without and once with `--features agent`: passed.
- `dprint check`: failed on SPEC.md and SPEC_impl.md, which one-helm-per-supervisor left unformatted. Fixed.
- The brick-test harness's tests (`python3 -B -m unittest discover -s releasing/mac-vm-test`): 277 passed
  (`2476ddca-d615-4ebe-8be6-cfbd14160787`).
- The vendored GPUI core's tests: passed (`1309823f-52a9-4d34-8f5e-4490021331b7`).
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`23619e63-3132-4115-a3de-1b8de2283a2d`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`1a1a1bf3-c564-4d63-bd5a-7f98ab136ff7`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`96f2b2cb-2797-4888-947c-a3d128988ac9`).
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  8068 tests, 4 failed (`5ffcbf0c-a1d7-46e1-8857-b06fcd598a8d`, `495fea20-f8ed-4fa3-9344-67ff5337a9fc`,
  `8ad794ef-8b95-4032-b700-e1f244bd2965`):
  - the macro compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat
    does not have, so it fails there every time; it was therefore not run this round, and nothing in the round touches
    it;
  - the Add host take-over test, broken by stacking these plans (see one-helm-per-supervisor's notes). Fixed;
  - the real Podman environment scenario, which ran past its 30-minute bound while sharing its boat with the rest of its
    shard;
  - a test that a scoped Stop leaves alone the jobs of a stock tmux server a session started, whose fixture found the
    pane job in the session's scope.

  The last two were re-run alone on boats on the same code and passed: the tmux test
  (`feaf27df-b63c-4a0b-acb6-8afb442eace0`) and the Podman scenario in about 11 minutes
  (`2ec35bdd-4a33-4e4d-9cff-07018e16c5d5`). Both failures look like load on the shared boat, not this round's code;
  their runs are kept. This is one observation of each, so neither is recorded as a flake.

On the fixed tree (one test's early keys, the specs' wording and formatting, and comments):

- `cargo fmt --all -- --check`, `dprint check`, `python3 scripts/check-agents-md.py` and
  `cargo clippy --locked -p farhelm-gui -p farhelm-proto -p farhelm --all-targets -- -D warnings`: passed.
- The Add host test module, locally: 40 passed, the take-over test included (`1db5cf13-ab3f-414c-9678-4e806893dd50`).

The rest was reused for the fixed tree: after it only that one test, the specs and comments changed.

Skipped:

- The Mac-only checks: no Mac-specific code changed.
