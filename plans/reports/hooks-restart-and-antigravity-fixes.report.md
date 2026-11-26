## What this was about

Antigravity could launch and complete a turn with Custom arguments ending in `--`, then fail to resume the saved
conversation. Its saved Restart command put the conversation selector where the CLI treated it as an operand. New
commands put the settings and selector before the delimiter, including when you select a named conversation.

A host's right-to-left hook setup error could reorder the surrounding Restart help and recovery message. The desktop now
draws the host's reason within the app sentence with its own text direction and displays hidden controls as visible
escapes. The CLI still gets the same escaped explanation.

Pressing Enter while Farhelm clones a checkout or runs its post-clone command could trigger a warning about a missing
conversation report before the agent was ready. Terminal input still works during setup. A submitted line starts the
timer once the agent takes over. After Restart, the timer waits for a new submitted line. If the agent fails to start,
Farhelm reports the startup error.

## Things you should know

Older Antigravity sessions keep the Restart command saved when they were created. Workspace trust still understands that
saved form. **Restart with** can give an older session a freshly compiled command with the corrected order.

An agent already running when you upgrade keeps its previous Enter timing until its next launch. If Farhelm cannot
record that a new agent is ready, the agent can still start and the missing-hook warning stays quiet for that launch.

The desktop right-to-left tests cover the text drawn in the menu and recovery card, the tooltip's shared description
source and the recovery group's accessible name. They use the GPUI driver and leave native screen-reader output
unverified.

## Open questions and possible follow-ups

There are no product decisions left open. The Antigravity tests cover command composition and the drift test's delimiter
check using fixtures. A vendor run of the current CLI would provide live confirmation.

## PRs

- [#5021 Keep Antigravity's Restart selector before Custom operands](https://github.com/scode/farhelm/pull/5021/changes)
- [#5024 Isolate hook errors in Restart and recovery](https://github.com/scode/farhelm/pull/5024/changes)
- [#5028 Start the missing-hook warning after agent handoff](https://github.com/scode/farhelm/pull/5028/changes)

## Checks run, reused and skipped

- `cargo nextest run --locked -p farhelm-helm -p farhelm --lib --test e2e -E 'test(/^launches::tests::/) | test(/^workspace_trust::tests::/) | test(=real_agent_capture::drift::permission_modes::antigravity_resume_options_must_all_precede_the_delimiter)'`
  passed 52 on `51614e75` (recorder `16f9c1c6-2ae5-461c-98be-d3ce5d708573`). The rebases left this change's interdiff
  empty and kept its compilation and trust logic the same. The delimiter check also passed in the final run.
- `cargo nextest run --locked -p farhelm-gui -p farhelm-proto --lib -E 'test(/^(session_list::restart::tests|session_pane::(header_tests|card_tests|card::tests)|launcher::restart_with_tests)::/) | test(persistent_hooks_explain_restart_readiness_from_the_launch_result)'`
  passed 128 on `571ed2d2` (recorder `bc20a1cc-0bd4-4af3-b8c8-3f4070f57720`). The launcher wording and CLI assertions
  still apply. The next run covers the revised desktop rendering and accessible names.
- `cargo nextest run --locked -p farhelm-gui --lib -E 'test(/^(host_text::tests|session_pane::(header_tests|card_tests))::/)'`
  passed 113 on `2e917b4b` (recorder `147d6ca4-f639-455c-b324-748c4e78de60`), including the two right-to-left reason
  tests. Opening the draft changed the commit description alone.
- `python3 scripts/check-gui-goldens.py` matched all Linux references in four jobs on `571ed2d2` (recorder
  `2a64e523-419c-40a8-a0b2-e2e0b0e4dc92`). The follow-up from source review leaves the recovery card's painted text the
  same, moves its accessible label to one owner and restores Replace's original text rendering. Targeted GUI tests cover
  those corrections.
- Final supervisor and real-host selection passed 381 on `06e99eaa` with all selected substrates exercised (recorder
  `de90d71e-0ae0-4988-b6fd-946129a28e20`). This covers the two repaired fixtures, the actual input route before and
  after handoff, the startup-error fence, store migration and reload, and the report acknowledgment path after its
  change landed on main. Opening the draft changed the commit description alone.

  ```sh
  cargo nextest run --locked -p farhelm-supervisor -p farhelm --lib --test e2e -E '
    test(/^(launch::tests|store::tests|hook_report::tests|service::(capture::tests|launch_artifacts::tests|notifications::tests|grid_stream::tests|core::tests::host_terminals))::/)
    | test(=host_creates::a_create_starts_its_agent_in_a_terminal_host)
    | test(=github_checkouts::a_raw_http_fresh_checkout_completes_the_real_clone_hook_and_agent)
    | test(=host_relaunches::a_restart_starts_a_new_host_that_keeps_the_history_on_a_blank_screen)
    | test(=host_relaunches::a_refused_replacement_keeps_the_previous_host_for_this_and_the_next_supervisor)
    | test(=hook_identity::watch_applies_first_report_before_a_long_ticker_interval)
    | test(=real_agent_capture::drift::permission_modes::antigravity_resume_options_must_all_precede_the_delimiter)'
  ```

- `cargo clippy --locked -p farhelm-supervisor --all-targets -- -D warnings` passed on `06e99eaa`. Scoped Rust
  formatting, test-delay lint, Markdown formatting, changelog formatting and whitespace checks also passed.
- The initial supervisor selection found two test fixtures that reused a launch path (recorder
  `36db7e1a-1b37-48ca-8edc-5bdba2b2b09d`). After updating them, the first rebased selection stopped at compile time on
  grid test calls that still passed raw input (recorder `161f1569-5a24-4221-a190-b09b045d67ef`). The final revision
  updates those calls and reruns the affected module.
- No native macOS GUI pass or live vendor turns. The Linux driver, goldens, launch fixtures and isolated host tests
  cover the changes this stack makes. The full workspace battery is left to the landing gate.

## Review gate

Each change received an independent GPT-6.2 Sol high source review. The Antigravity reviewer had no findings. The
Restart wording review found a duplicate accessible name on the recovery card and unexpected labels on Replace, both
corrected and retested. The handoff review found repeated fixture paths, an exec failure that could still warn, metadata
lookups on input that cannot submit a line and a paste wording mismatch. The final revision addresses those findings and
passes the selected tests.

### Landing

#### What landed with it

This plan landed first in its round; the release brick test's retry and reopen fixes
(`brick-test-reopen-and-retry-fixes`) landed right after it and touch only the brick test's own tools. Since this plan's
PRs were based, main gained the Settings triage round. The rebase applied without conflicts. The landing's independent
review also checked the earlier changes this plan builds on (the failed Restart cleanup retry, the hook report
acknowledgments and the Welcome empty pane) and found no interaction: the startup cleanup that reclaims orphaned
terminal hosts never sees the new handoff marker, both the failed Restart cleanup and the startup sweep remove it, and
the session header and card changes sit on top of the Welcome change.

#### Fixes made while landing

- **One Delete test failed on every run with the plan.** The test that checks Delete refuses when a launch file cannot
  be removed first clears the launch's other files so that exactly one stays blocked, then checks that only that one is
  left. The plan's new handoff marker is another such file, so the check failed (test-worker recorder run
  `5185ff46-5722-4079-b858-27e3a2828d51`; the executor's focused runs did not include this test). Fixed inside
  [#5028](https://github.com/scode/farhelm/pull/5028/changes): the test clears the marker too, as it clears the others;
  Delete itself was right. The landing's independent review found no blocker or should-fix item.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on both plans' PRs stacked in queue order on main as it stood at the start of the landing (`62848f0d`), which applied
without conflicts; every file's change was exactly what the executors pushed until the one test fix inside #5028.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `5185ff46-5722-4079-b858-27e3a2828d51`), before the fix: 8743 of 8744 passed, with no runtime `SKIPPED` lines. The one
  failure was the Delete fail-closed test, fixed inside #5028 as the Restart and hook warning plan's "Fixes made while
  landing" describes.
- After the fix, that test five times in a row,
  `cargo nextest run -p farhelm --test e2e -E 'test(=session_lifecycle::delete_fails_closed_when_a_launch_artifact_cannot_be_removed)'`,
  on the test worker (recorder runs `1b63cc37-c937-48e3-af55-68360ef893a6`, `aa7fc04d-ba16-4bbc-8063-d5ecb210951f`,
  `1a41ad24-07bf-4d0c-8466-0813a9d733b1`, `2dc96b26-8956-47b4-9e8c-009deb499ee9` and
  `d2cbf0dd-7a20-4515-835e-d07680dda566`): passed every time; and its whole module,
  `cargo nextest run -p farhelm --test e2e -E 'test(/^session_lifecycle::/)'` (recorder run
  `2690ad56-caba-4088-ae38-3a4f23956e80`): all 80 passed. The fix changes only that test's setup, so the rest of the
  workspace run still applies.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `8cb7bdf6-b1be-452c-95cd-56b25b487ab0`): of its 49 steps, 48 passed and one was skipped by design on Linux (paste),
  including Restart.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `204aa932-5b38-4989-8d30-1b97d963a72c`): passed, including the session header and card scenes.
- The release brick test harness's portable tests,
  `python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py'`, locally (recorder run
  `307cd69e-d7f1-400c-b154-a64c3f19997a`): all 332 passed.
- `python3 scripts/check-agents-md.py`, `python3 releasing/check-changelog.py format`, `cargo fmt --all -- --check`
  (before and after the fix) and `python -B scripts/check-test-sleeps.py`, locally: passed. `dprint check`, locally:
  these plans' files pass; it fails only on over-long lines in the triage outcomes archive, which main has had since an
  earlier triage record (#4978) landed. No PR fixes them yet; a formatting-only one would, and I recommend it, as
  earlier rounds' notes did.

Reused across the fix: every run above apart from the reruns named. The executors' runs were focused selections on older
bases, and everything they covered ran again above.

Skipped: the tests of the desktop app's outer shell (`cargo nextest run -p farhelm-desktop`), since nothing in it
changed (the app's windows, including the session header and recovery card, live in its GUI crate, whose tests ran in
the workspace suite); a real Mac brick test run, which is a release-time check (the brick test plan's report says what
it still has to show); a native macOS pass of the right-to-left hook error text and its screen reader output, which the
Restart and hook warning plan's report leaves unverified; a live Antigravity run, which spends vendor turns; and CI,
which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **The missing-hook warning can still start before the agent is really ready in a few setups.** The plan starts it once
  Farhelm hands the terminal to the agent command, but a custom agent's wrapper, a container image pull or a remote
  clone in a sandbox can still run after that point. This is a remaining gap, not a regression.
- **Smaller points the review noticed.** Until the handoff, each keystroke in a hooked agent's terminal costs one file
  check on that terminal's connection, which a code comment describes as unable to hold up other terminals, more
  strongly than it can promise; SPEC.md's rewritten paragraph no longer says outright that Pi and OMP are never warned;
  and one changelog draft presents hidden characters showing as visible codes as new, which they already did. None needs
  a follow-up on its own.
