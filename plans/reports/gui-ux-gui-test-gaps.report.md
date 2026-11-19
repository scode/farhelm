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
