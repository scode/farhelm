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
