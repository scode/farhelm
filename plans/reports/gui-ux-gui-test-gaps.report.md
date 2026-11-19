## What this was about

Five approved findings from the November 18 GUI UX triage concerned tests and their fixtures. A test could pass without
proving that a recreated Claude login reached the sandbox, that Restart with used the catalog after a retry, or that a
permission warning appeared before a later switch cleared it. The model picker height test could accept a collapsed
list. Two portable release fixtures could send Git setup commands to a repository inherited from a Git hook.

The maintainer chose small changes that reuse the existing tests and fixtures. Each PR records the outcome it covers and
removes its feedback entry from the queue. All five are ready for the landing monitor.

## Things you should know

If delivery matched Claude logins only by name and revision, a sandbox could keep the old login after deletion and
recreation. The new test leaves the old successful delivery record in place through recreation and verifies that
reconnecting delivers the new login. Both logins have the same name and revision.

In Restart with, Retry clears the error before a reply arrives. The test now requires Low to appear as an available
effort once the catalog arrives. The actual restart still sends the stored model and High effort together with the
Guarded permission selected before Retry. The warning test starts with Guarded and switches to OpenCode, which requires
YOLO. It verifies that this warning appears before a switch to Codex clears it. The model list test verifies that the
catalog supplied 30 models and that the list fits a complete first row within its height cap.

Each release fixture removes inherited Git variables from the environment it passes to its setup writes, which the
approved finding identified as the risk to another repository. A portable test started from a Git hook can still fail
when the coordinator's later read-only Git lookup follows an inherited selector. The isolation probe measures the setup
writes with a controlled lookup to verify where they actually went.

## Open questions and possible follow-ups

None.

## Pull requests

- Check that reconnect delivers a recreated Claude login. [#4207](https://github.com/scode/farhelm/pull/4207/changes)
- Check that Restart with uses the catalog after Retry. [#4209](https://github.com/scode/farhelm/pull/4209/changes)
- Verify the permission warning before switching agents again.
  [#4212](https://github.com/scode/farhelm/pull/4212/changes)
- Require enough height for the first model-list row. [#4217](https://github.com/scode/farhelm/pull/4217/changes)
- Isolate the release fixtures' Git writes. [#4219](https://github.com/scode/farhelm/pull/4219/changes)

## Checks

- `cargo nextest run -p farhelm-helm --lib -E 'test(recreated_claude_login_) | test(claude_subscription_reconciles) | test(recreated_codex_login_)'`
  passed all four cases through the recorder (`af805bcc-9ac4-43d8-ab5c-4e4bdff167ca`). Blanking only the Claude delivery
  stamp's creation identity made the new-token assertion fail (`0b4d374d-88e0-4337-b9af-27dbe1b4138b`, test candidate
  `0e424902`). The mutation run used the same command with only
  `test(recreated_claude_login_replaces_the_previous_subscription_stamp)`. Review confirmed the final test keeps the
  failing assertion and its added premises observe fields independent of the mutation.
- `cargo nextest run -p farhelm-gui --lib -E 'test(=launcher::restart_with_tests::the_catalog_retry_keeps_an_edit) | test(launcher::escalation_tests::) | test(=launcher::view_tests::the_model_list_stops_at_260_pixels)'`
  passed all 12 selected tests through the recorder (`2602b0e2-c78a-47b9-8fe8-85476004b092`). The mutation run
  (`67f81e73-d17a-4fa8-972a-151fa149be30`) used the same command with the escalation module selection narrowed to
  `test(=launcher::escalation_tests::a_later_switch_clears_the_banner)`. The three temporary edits ignored the fixture's
  catalog reply, withheld the warning and collapsed the list. Each exact test failed on its new assertion. The clipped
  list measured 14 pixels for a 41-pixel row. I restored all three temporary edits before the final run.
- `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v` passed all 277 portable tests (recorder
  `b2d910bd-7378-408f-b5c8-6c6a52852e56`). A separate two-case probe through the recorder used
  `env GIT_DIR=<disposable-bare-git-dir> PYTHONPATH=<fixture-module-directory> python3 -B <fixture-write-probe.py>`. It
  exercised the release-tag fixture and guest source fixture and checked that each created its own `.git`. Both fixed
  fixtures passed (`cd2d6414-bf78-4736-b73e-dd3794f27cdc`). Repeating with the modules from base `9ed244e2` first on
  `PYTHONPATH` failed both checks (`280a0207-00bc-439f-b8fb-8c95898a40e8`). An earlier probe also discovered imported
  cases that used the ordinary read-only path (`bc870854-a515-40e2-86a1-f3ad0acd2875`). The corrected probe selected
  only its two write checks.
- `cargo fmt --all -- --check`,
  `dprint check --incremental=false TRIAGE_OUTCOMES.md TRIAGE_OUTCOMES_ARCHIVE.md review_feedback_queue/INDEX.md`,
  `git diff --check` and `python -B scripts/check-test-sleeps.py` passed. The index matched all 841 feedback files.
  `python3 scripts/check-agents-md.py` passed with two existing citation warnings. Clippy passed with
  `cargo clippy -p farhelm-helm -p farhelm-gui --all-targets --locked -- -D warnings`.
- Skipped real Mac guests because the touched fixtures run in the portable suite. Desktop smoke and hosted CI would
  repeat coverage outside the test and fixture paths changed here.

## Review gate

Each PR passed its independent source review and cold commit-message read. Review tightened the Claude stamp, pre-Retry
permission and populated list premises. It also checked that each PR carries the corresponding queue removal and
preserves the approved outcome in the archive.

### Landing

#### What landed with it

It landed in a round of five plans: drawn-frame-actions, one-helm-per-supervisor, gui-ux-discards,
gui-ux-backend-test-gaps and gui-ux-gui-test-gaps. Since its base, main gained the previous rounds' plans (Shift+drag
marking, model family words, unusable-folder refusals and earlier ones) and bookkeeping. It shares a test file with
drawn-frame-actions, which changes a different test in it; its assertions go through the shared click helper and are
unaffected by that plan's change to it.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The five test changes each
check their premise and would fail if the behavior broke. The release fixtures now strip Git's environment variables
only for their own setup writes, as the outcome asked.

#### Fixes made while landing

None.

#### Things you should know that the report does not say

- The model list height test is weaker than its name: its new lower bound passes any list at least one row tall, so it
  would not catch a list capped well below 260 pixels.
- One test's step-by-step "Specified:" docstring was replaced with a vaguer summary.

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
