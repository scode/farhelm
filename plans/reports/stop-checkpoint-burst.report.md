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

### Landing

Landed with one test fix at landing, together with readopt-unavailable-hosts, which was stacked below it in the same
round. An orderly supervisor stop now saves the terminals with the most new output first, uses at most half the cores
for it, and leaves terminals with less than 1 MiB of new output for the next supervisor to replay.

- #2952: the largest-first order and the cap on concurrent saves.
- #2954: skipping the save for small output tails.

#### What the landing fixed, and why

An existing test, that a restarted supervisor keeps a terminal tab attachable with its screen, asserted that the orderly
stop left the tab's host a snapshot. With #2954 a tab that has printed only one short line gets no snapshot at the stop
(the next supervisor replays its output instead), so the test failed on the combined stack. Neither executor had run it
on code with #2954: this plan's executor did not run the tab tests, and readopt-unavailable-hosts' executor ran them
without this plan. The landing gave that test the same setting this plan already gave its own restart tests, saving
every tail at the stop, so it still tests recovery from a stop's snapshot; no product code changed. The other way would
have been to keep the shipped setting and check that the tab comes back by replaying its output. As landed, no tab test
runs an orderly stop under the shipped skipping of small tails; adding one would be a follow-up.

#### What landed since the stack was based, and whether it interacted

Since the plan's base, main gained the grid terminal's link fix (#2949: no product code outside the browser, plus a
supervisor test and protocol comments) and four PRs that only added TODO entries and plans; none interacts with this
plan; two more such PRs (#2962, #2966) landed while the round's checks ran. readopt-unavailable-hosts, landed just
before this plan, rewrites the same supervisor code for recovering unavailable terminals; a fresh-context review read
the two against each other and found no harmful interaction (its landing notes say what was checked). The whole Rust
workspace ran on the combined stack (below). The rebase conflicted only in TODO.md and in one implementation-notes
paragraph both plans edit in different sentences; both were resolved keeping each side's changes, and a check confirmed
each commit changes exactly the sentences it changed when tested.

#### Things you should know

- The TODO entry about typing into a new terminal pausing while its checkpoint is encoded (planned as
  faster-snapshot-encoder) still says every orderly stop saves a checkpoint and points at the entry this plan removed.
  That plan is in flight and changes its own entry when it lands, so the landing left it alone.
- The validation inventory's selection for terminal hosts across supervisor restarts does not list this plan's new
  tests, and its advice to run it when the orderly stop changes does not mention the tab tests, which is how the failing
  tab test went unrun. Updating it would be a follow-up.
- The whole workspace run had three failures that also failed, the same way, in one run on main without either plan, in
  code neither plan changes (the checks below say what they cover).

#### Checks

This round landed readopt-unavailable-hosts and stop-checkpoint-burst, stacked in that order on main `e499b218`. On that
stack:

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, the check that every test delay states its reason
  (`python -B scripts/check-test-sleeps.py`, 403 delays, none without a reason), `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no errors; three warnings about
  older reports) and the workspace debug build: all passed.
- The whole Rust workspace, `cargo nextest run --workspace --exclude farhelm-desktop`, with the pinned test runner and
  tmux, no retries, on a machine with rootless Podman, run `7670d6ed-f91a-4c68-a7ca-38594df84b0f`: 5412 passed, 69 left
  out by the test runner (tests marked ignored), 5 failed. "A restarted supervisor keeps a host tab attachable with its
  screen" is the test the landing fixed (stop-checkpoint-burst's notes). "Shallow pause resumes without reset or replay"
  is a recorded open flake, in terminal code neither plan touches. Three more failed the same way in one run on main
  `e499b218` without either plan (run `03a4af08-649b-48ca-88bb-5c5adf9d199d`, in which the tab test, as a control,
  passed), so they are not this round's. Two cover sessions whose tmux session or server died across a supervisor
  restart, which must stay listed, as exited, while attaching to them fails ("restart gap is decided per session" and
  "restart gap lists sessions without a terminal and attach fails"); in both, the listing no longer names tmux as the
  session's agent terminal where the test expects it. The third, a supervisor unit test, covers an OMP agent's report
  that arrives before its session's pane is known ("OMP publication gap parent report succeeds"), which was refused with
  "the session's foreground pane is unavailable". Each failed fast, within about a second, in both runs. They are in
  neither FLAKES.md nor TODO.md; the landing monitor told the maintainer in conversation, and finding the cause,
  starting with a narrow rerun, is a follow-up.
- After the landing's test fix: `cargo fmt --all -- --check`, `cargo clippy -p farhelm --all-targets -- -D warnings`,
  the test-delay check and `dprint check` passed, and the end-to-end `host_tabs::` and `host_restarts::` modules with
  the supervisor's `core::tests::host_terminals::` unit module
  (`cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E '...'`), run
  `a086105a-8f62-4573-ac14-924962a8644c`: all 83 passed.
- Reused: nothing. Not run: the browser, desktop and JavaScript tests (no user interface code changed) and the doctests
  (no doc examples changed).

The executor's report names its commits before the landing's rebase and test fix, and calls the PRs drafts; what landed
is those commits rebased, with the test fix added to #2954's. Its test list does not include the tab test the landing
fixed. Nothing else in it became untrue at landing.
