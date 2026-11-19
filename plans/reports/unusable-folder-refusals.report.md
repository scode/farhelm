## What this was about

Creating or restarting a session, or opening a tab, could fail with an internal program name when the host could see the
session folder but could not enter it. A terminal viewer requesting zero rows or columns could take over the terminal
and connect at its old size. Six relevant tests were lost when Farhelm stopped creating tmux terminals.

The maintainer chose to refuse both requests up front, name the folder when the host cannot enter it, and restore the
tests against terminal hosts.

## Things you should know

The refusal names the working folder and leaves the session or tab unstarted. Restart checks the folder before trying to
verify the saved conversation. The preflight uses the effective user on the host. The host still has to enter the folder
when it launches, so a permission change in that interval can still produce its own launch error.

A request with a zero dimension returns a clear error before the supervisor looks up a terminal or displaces the current
viewer. The current viewer keeps receiving output and retains its terminal size. A zero-size resize on an existing
connection still warns in the log, and the starting size at create is still clamped.

The restored tests also cover memory growth while terminal output continues behind a paused viewer, upload
acknowledgements passing queued terminal history on the same connection, and finishing an upload after the active agent
causes a guarded Delete to be refused.

## Open questions and possible follow-ups

None.

## Pull requests

- Name an unenterable working folder before launching. [#4188](https://github.com/scode/farhelm/pull/4188/changes)
- Reject a zero-size attach before it can displace the current viewer.
  [#4190](https://github.com/scode/farhelm/pull/4190/changes)
- Restore the three backpressure and upload tests and remove the completed TODO entry.
  [#4194](https://github.com/scode/farhelm/pull/4194/changes)

## Checks

All runtime checks below ran locally through the test recorder on Linux. The e2e binary used real terminal hosts and a
real supervisor. Every selection used `cargo nextest run -p farhelm --test e2e -E` with the filter shown.

- Eleven working-directory and path cases passed on the folder draft (run `0a600cd2-2c40-4e5b-947a-5e942704e37c`). The
  two restored cases also checked the point when a terminal host starts. Filter:

  ```text
  test(a_directory_it_cannot_enter) | test(missing_directory) | test(working_directory) | test(keeps_startup_cd)
    | test(create_with_relative_cwd) | test(create_with_nul_byte_in_cwd) | test(create_in_a_regular_file)
  ```

- In the first size-guard run, eight existing cases passed and the new regression stopped during setup. It expected a
  space after the fake agent's prompt, which the grid omits (run `88ec50cd-b5e4-477e-b652-c6199cde5567`). After
  correcting the prompt check, `test(=session_lifecycle::attaching_at_zero_size_leaves_the_incumbent_and_its_size)`
  passed (run `fd184b37-d88b-4819-904c-27fbc47ea30b`). Filter for the first run:

  ```text
  test(attaching_at_zero_size) | test(attach_replay_uses_the_requested_geometry) | test(attach_to_unknown_session)
    | test(attachment_channels_must_be) | test(over_cap_lease) | test(resize_from_) | test(second_attach_detaches)
    | test(attach_under_a_different_lease) | test(attach_racing_a_stop)
  ```

- Final upload-priority and stalled-viewer cases passed (run `314d1eb2-66aa-4817-ae7a-5c4041fc8382`). The RSS case
  reserved all four nextest slots and measured the terminal host and the supervisor. The paused terminal processed 18
  MiB over seven accepted-checkpoint samples spanning about 15 seconds. The host's peak grew by about 540 KiB, and the
  supervisor's by about 128 KiB. The original limit is 8 MiB per process. Filter:

  ```text
  test(=terminal_backpressure::memory_stays_flat_while_a_viewer_is_stalled)
    | test(=attachment_uploads::an_ack_arrives_ahead_of_a_backlog_of_terminal_output)
  ```

- The guarded-Delete upload test and both process-sampler parser tests passed on the earlier retained run
  `f3c4c694-2cc1-43b9-87a8-3cd2b5387e73`. Those tests are unchanged in the final draft. The earlier RSS run timed out
  while waiting to replay its final marker after measuring. The final run checks resumed output and the producer's stop
  acknowledgement independently of replaying the full backlog. Earlier filter:

  ```text
  test(terminal_backpressure::) | test(a_refused_delete_preserves_an_in_flight_upload)
    | test(an_ack_arrives_ahead_of_a_backlog_of_terminal_output)
  ```

- `cargo clippy -p farhelm-supervisor -p farhelm -p farhelm-fixtures --all-targets --locked -- -D warnings` passed.
  Rust, Markdown and TOML formatting, changelog format, whitespace, and test-delay checks passed.
- I skipped the workspace battery, desktop smoke and hosted CI because these changes are in supervisor admission and
  focused e2e tests. The desktop continues to receive refusals through its existing routes.

## Review gate

Each draft passed its independent source review and cold commit-message read. I moved the directory check ahead of
saved-transcript verification, used a host-start counter in the folder tests, and made the zero-size test check a
nondefault terminal size and the order of validation. The restored tests verify a paused stream, compare memory at
accepted checkpoints, and measure upload priority against an already queued replay prefix.
