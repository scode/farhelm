## What this was about

During an update or supervisor replacement, shutdown could spend every core encoding full terminal snapshots, even for
terminals with only a few new bytes. Checkpoints competed for capacity in arbitrary order, so the stop's deadline could
expire before saving the terminals with the most output to replay.

The agreed policy is implemented: shutdown admits the largest output tails first, uses at most half the reported cores
for its own checkpoint requests (at least one), and leaves tails below 1 MiB for the next supervisor to replay. The
ten-second deadline, encode cancellation and the fence against publishing new connected terminals during shutdown are
preserved.

## Things you should know

Skipping a checkpoint preserves the previous snapshot and recent output; it does not discard terminal history. A skipped
terminal's connection closes immediately, before other checkpoints finish. The runtime checks again when it handles a
queued request, because an automatic checkpoint may have finished meanwhile and made another full encode unnecessary.

The stop threshold is separate from ordinary automatic checkpointing. It is the smaller of 1 MiB and one quarter of the
host's retained-output limit: how much raw output it keeps after a snapshot. Normal hosts keep up to 32 MiB and
therefore use the 1 MiB threshold; smaller limits are exercised by test configurations. The addressed TODO entry was
removed and the existing changelog fragment updated. Encoder speed and its separately planned optimization are
unchanged.

## Open questions and possible follow-ups

Automatic encodes already running, or started by terminals still waiting for shutdown capacity, remain outside the
stop's slot cap, as agreed. Counting those would require changing normal automatic checkpointing during shutdown; this
plan does not make that policy change. No unresolved decision remains.

## The PRs

- [#2952: reduce checkpoint contention at shutdown](https://github.com/scode/farhelm/pull/2952/changes), draft, commit
  `18c5a664d7a9cab1a3c44a143e7a655beb160f3a`.
- [#2954: skip shutdown saves for small output tails](https://github.com/scode/farhelm/pull/2954/changes), draft, commit
  `9d3765cc197f18c65ab905270cefb5f78d800e39`, based on #2952.

## Checks run, reused and skipped

- PR1 shutdown ordering and supervisor-restart selection: 20 tests passed, recorder
  `a89ae8eb-9716-4fe5-b632-92f072a382da`, against the working tree committed as `18c5a664`.
- PR2 host-terminal runtime, real-host recovery, shutdown and supervisor-restart selection: 94 tests passed, recorder
  `7f992133-6977-450f-957e-cc7905bcf61e`, against the working tree committed as `9d3765cc`. This includes the exact
  threshold, skip before capacity is available, automatic-encode race, ordering and cancellation tests.
- Both runs used pinned nextest and tmux, four slots and zero retries. Complete retained output contains no runtime
  `SKIPPED` marker. Filtered-out cases were outside the selections; the recorder also reports each successful fixture
  setup separately.
- For each PR, `cargo clippy -p farhelm-supervisor --all-targets -- -D warnings` and
  `cargo clippy -p farhelm --bins -- -D warnings` passed. Rustfmt, changed-file dprint and changelog format passed. The
  source delay checker passed for both, inspecting 397 delays with no missing rationale.

The portable runtime commands below ran from the checkout root, with the repository's pinned nextest and tmux first on
`PATH`. The recorder enforces four slots and zero retries; the build used four jobs.

PR1:

```sh
CARGO_BUILD_JOBS=4 python3 scripts/record-test-run.py --runner nextest --kind development --selection 'PR1 shutdown ordering and host restarts' --concurrency '4 nextest slots; retries 0; 4 build jobs' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E 'test(/a_prompt_waits_for_a_recovered_host_terminal_to_catch_up|spare_claims_read_a_host_rows_own_host|a_stop_request_survives_a_restart|an_unreadable_launch_record_leaves_its_session_unavailable|an_orderly_stop_ends_its_encodes_before_its_runtime_drops|a_readoption_that_loses_its_fence|readoption_backs_off_without_spinning|a_scan_restricted_to_some_names|delete_settles_the_record_of_a_startup_placeholders_socket/) | test(/^host_restarts::/) | test(an_orderly_stop_admits_largest_tails_first_with_one_slot) | test(exit_checkpoints_use_half_the_cores_with_at_least_one_slot)'
```

PR2:

```sh
CARGO_BUILD_JOBS=4 python3 scripts/record-test-run.py --runner nextest --kind development --selection 'PR2 host runtime, shutdown skips and restarts' --concurrency '4 nextest slots; retries 0; 4 build jobs' --tmux required -- cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e --test terminal_host_runtime -E 'test(/^hosts::terminal::/) | binary(terminal_host_runtime) | test(/a_prompt_waits_for_a_recovered_host_terminal_to_catch_up|spare_claims_read_a_host_rows_own_host|a_stop_request_survives_a_restart|an_unreadable_launch_record_leaves_its_session_unavailable|an_orderly_stop_ends_its_encodes_before_its_runtime_drops|a_readoption_that_loses_its_fence|readoption_backs_off_without_spinning|a_scan_restricted_to_some_names|delete_settles_the_record_of_a_startup_placeholders_socket/) | test(/^host_restarts::/) | test(an_orderly_stop_admits_largest_tails_first_with_one_slot) | test(exit_checkpoints_use_half_the_cores_with_at_least_one_slot) | test(an_orderly_stop_skips_small_tails_before_waiting_for_a_slot) | test(a_failed_host_restart_after_a_stop_began_restores_a_placeholder)'
```

No earlier-session test evidence was reused. The checks covered the source trees committed in the two PRs. A final main
audit found only plan bookkeeping, with no functional interaction, so no rebase or additional runtime run was warranted.
Browser, desktop, full-workspace and CPU-burst benchmarks were skipped: the changed behavior is backend shutdown policy,
and deterministic gates plus recovery tests directly exercise it. No end-to-end update speedup was measured.

## The review gate's outcome

Each PR passed a separate fresh-context gpt-6.1-sol high adversarial review, including the full test-authoring
checklist, with no findings. Both commit messages received valid cold reads.
