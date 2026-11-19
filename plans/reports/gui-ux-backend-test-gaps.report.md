### What this was about

The November 18 triage found fifteen gaps in tests for the supervisor and terminal hosts. Some tests could pass without
observing the result they claimed to check. Others could wait past their own deadline, fail when the test and the
terminal host used different tools, or leave a helper running after the test. The maintainer asked for the smallest
clear fix in each case, with a discard allowed if the fix needed a new test mechanism.

### Things you should know

All fifteen fixes fit in the existing tests and fixtures. The tests now distinguish a canceled terminal request's late
reply from the current reply and wait until the supervisor has processed a rejected checkpoint before verifying the
saved state. They require the retirement warning to remain visible as resolved and a shell tab to answer a fresh command
after its supervisor restarts. Other cases verify that a particular startup helper reached its gate or inherited the
version lock before the test continues. The listing and listener each have a deadline.

The terminal settings test measures both terminals with the same `stty` program. The local snapshot test uses the
workspace's JSON parser to read resize records with valid field spacing. The changes cover tests, fixtures and their
queue records.

### Open questions and possible follow-ups

None.

### PRs

- Verify that a canceled terminal request's late reply leaves the active request waiting.
  [#4211](https://github.com/scode/farhelm/pull/4211/changes)
- Use a childless helper in the terminal socket presence test.
  [#4213](https://github.com/scode/farhelm/pull/4213/changes)
- Check saved state after the host's checkpoint rejection has been handled.
  [#4214](https://github.com/scode/farhelm/pull/4214/changes)
- Apply the tab cleanup test's deadline to the listing request.
  [#4215](https://github.com/scode/farhelm/pull/4215/changes)
- Wait for the intended startup to reach its gate before launching another session.
  [#4218](https://github.com/scode/farhelm/pull/4218/changes)
- Use the same terminal settings program for the host and reference measurements.
  [#4220](https://github.com/scode/farhelm/pull/4220/changes)
- Require the notification to exist after migration before testing its deletion.
  [#4221](https://github.com/scode/farhelm/pull/4221/changes)
- Wait for the replacement supervisor to process the program's recorded exit.
  [#4222](https://github.com/scode/farhelm/pull/4222/changes)
- Require a reattached shell tab to answer a command after restart.
  [#4223](https://github.com/scode/farhelm/pull/4223/changes)
- Check that the terminal starter inherits the version lock. [#4224](https://github.com/scode/farhelm/pull/4224/changes)
- Verify a live process still holds stderr when terminal startup reports ready.
  [#4225](https://github.com/scode/farhelm/pull/4225/changes)
- Bound the startup cleanup test's listener wait. [#4226](https://github.com/scode/farhelm/pull/4226/changes)
- Verify that the first startup helper has exited and been reaped before reporting ready.
  [#4227](https://github.com/scode/farhelm/pull/4227/changes)
- Parse resize records as JSON in the local snapshot fixture.
  [#4228](https://github.com/scode/farhelm/pull/4228/changes)
- Require the resolved warning about the retired terminal after supervisor restart.
  [#4229](https://github.com/scode/farhelm/pull/4229/changes)

### Checks run, reused and skipped

- Passed 16 selected tests on Linux. Five supervisor cases passed in `fdde0fdb-f055-4d64-80ca-203879c692f3`. The earlier
  socket fixture failed there and passed with its final childless helper in `3d77cf7c-2237-4920-ab1f-9f680108c659`.
  Eight integration and process tests passed in `145653fa-b40f-4f51-b33c-ffc0cab8546b`. Separate tests passed with an
  alternate `stty` first on the runner's PATH in `9516312f-2e43-4888-a403-5668ec467385` and with a synthetic JSON
  recording in `bb1e01a8-511e-41c8-b6bd-7ed7ee3c1b40`. The Linux integration and process runs exercised their required
  runtime prerequisites, including real terminal processes and the supervisor restart. No selected test skipped its
  runtime check.
- Fourteen temporary fault injections failed at the intended checks. They covered stale replies, rejected checkpoints,
  missing notification rows, a stalled listing or listener, unobserved launch gates, a missing shell reply, a missing
  inherited lock, a simulated observation that the first startup helper was still running, an early exit of the process
  holding terminal error output open, disappearance of an old warning, the old recording parser, and mismatched `stty`
  tools. The two interrupted-start cases (cleanup with a surviving host and fencing a start that never ends) returned
  within their deadlines, and their helpers exited. The tested source was restored byte-for-byte after each trial.
- Passed
  `cargo clippy --locked -p farhelm-supervisor -p farhelm -p terminal-host -p farhelm-terminal-emulator --all-targets -- -D warnings`,
  Rust and scoped Markdown formatting, whitespace, and the test-delay checker. The agent-instruction checker found no
  errors and repeated two pre-existing heading warnings.
- Reused the selected test evidence at draft stack tip `bf7a8be9`. Its source diff from `05acb7c6` matches the tested
  changes byte-for-byte. Skipped the workspace-wide test suite because the changes are confined to the named tests and
  fixtures. The Linux test reproduced the tool mismatch with a substitute `stty`. Native macOS was untested. The
  snapshot test used a synthetic recording.

The recorded positive runs used these `cargo nextest run` selections through `scripts/record-test-run.py`. The last run
kept `FARHELM_SNAPSHOT_RECORDINGS` pointing to the synthetic recording, and the PTY run prefixed `PATH` with a
substitute `stty`.

```sh
cargo nextest run -p farhelm-supervisor --lib -E 'test(/a_dropped_requests_reply_is_skipped|a_socket_a_process_names_is_never_removed|checkpoints_are_taken_at_clean_reads_once_due|schema_34_drops_tmux_names_and_keeps_every_session_and_reservation|readiness_does_not_wait_for_stderr_to_close|an_unfinished_start_ends_a_host_that_answers/)'
cargo nextest run -p farhelm-supervisor --lib -E 'test(=hosts::registry::tests::a_socket_a_process_names_is_never_removed)'
cargo nextest run -p farhelm --test e2e --test terminal_host_entry --test terminal_host_spawn -E 'test(/an_exited_host_tab_is_reaped_on_its_hosts_exit|an_interrupted_start_of_a_missing_session_is_settled_and_its_host_left|an_interrupted_start_that_never_finishes_is_fenced_and_its_record_cleared|same_boot_classification_is_per_session_and_never_interrupted|a_supervisor_restart_leaves_a_tabs_shell_and_scrollback_untouched|a_dead_old_agents_scoped_job_is_ended_and_the_session_restarts_in_a_host|a_terminal_host_holds_no_version_of_farhelm|a_spawned_host_is_listening_and_its_first_stage_is_reaped/)'
cargo nextest run -p terminal-host --test host_process -E 'test(=the_terminal_settings_are_a_new_ptys_with_erase_and_iutf8)'
cargo nextest run -p farhelm-terminal-emulator --test snapshot_roundtrip --run-ignored only -E 'test(=private_recordings_round_trip)'
```

The fault-injection runs used the same commands with smaller selections where appropriate. The supervisor selection
omitted socket presence in `6897e87f-9751-4d36-803c-667f387bf348` (four expected failures). A separate
`cargo nextest run -p farhelm-supervisor --lib -E 'test(=hosts::spawn::tests::readiness_does_not_wait_for_stderr_to_close)'`
verified the fifth supervisor failure in `3f69bd68-7a8a-4475-a02e-63e5622b5277`. The integration and process runs
omitted the boot classification case in `9f306539-7dc3-4491-a091-5c60220b9280` (seven expected failures). The old JSON
parser and the mismatched `stty` failed with their corresponding selections in `c957b105-c94f-4924-87ef-8465295d8be0`
and `c042b30b-70d8-4c6b-803b-89efdde72eb4`.

### Review gate

One fresh high-effort GPT-6.2 Sol reviewer checked each change. The changes address the source findings within the
agreed fixture scope. Commit messages received independent cold reads.

### Landing

#### What landed with it

It landed in a round of five plans: drawn-frame-actions, one-helm-per-supervisor, gui-ux-discards,
gui-ux-backend-test-gaps and gui-ux-gui-test-gaps. Since its base, main gained the previous rounds' plans (Shift+drag
marking, model family words, unusable-folder refusals and earlier ones) and bookkeeping. It shares two files with
one-helm-per-supervisor, which adds a supervisor schema version; its new test of notification cleanup runs after
migration to the latest schema, so the two do not interact.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. Each of the fifteen test
changes now checks its premise and would fail if the behavior it covers broke. None adds an unexplained sleep. The
socket presence fixture fix is about 25 lines against an estimate of 5 to 10, within the plan's allowance of 30 lines
for one fix.

#### Fixes made while landing

- **A test lost the only explanation of its design.** The boot-id durability test's exits deliberately happen without
  listing sessions first, because a listing is how the supervisor records an exit, which would test the wrong path. The
  PR replaced that with a sentence relying on an undefined term; it is back.
- **A new test dependency had no comment.** The terminal emulator's tests now read recordings as JSON; a comment says
  why.

#### Things you should know that the report does not say

- The shared polling helper now fails a condition that succeeds just after its deadline, with a message saying the
  observation exceeded the deadline. That is what the outcome asked for (the deadline bounds the in-flight check too),
  but the message reads oddly when the last check succeeded.
- A few comments wrapped at 80 columns now have long lines added, and one in the terminal host start tests lost its
  explanation that the first stage that forked a process has exited. I left them.

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
