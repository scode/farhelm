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

### Landing

#### What landed with it

It landed in a round of three plans: shift-drag-marking, model-family-shorthand and unusable-folder-refusals. Since its
base, main gained earlier rounds' plans (session group dragging, model families, the covered-link refusal, the demo
video recorder, the Replace with checkout hint and others) and bookkeeping. Its changes are on the supervisor side only,
so they do not meet the two launcher and session list plans. Its refusal of a connection to a terminal with zero rows or
columns is checked before a new connection can take the terminal over from its current viewer (which an earlier plan
changed), so a refused connection disconnects nobody.

At your request this landing was reviewed with extra skepticism, by me and by independent reviewers, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The six tests dropped when
terminals moved into terminal hosts are back, each checking what it checked before, and the refusal wording is the form
the launcher already recognizes to mark the folder field.

#### Fixes made while landing

- **The new folder check could refuse every launch inside some containers.** It asked the kernel with a flag that makes
  the system library use a newer call (`faccessat2`), which some older container security profiles answer with "not
  permitted" instead of "no such call". The check would read that as a permission problem and refuse every create,
  restart and tab open with "working directory is not usable", and supervisors do run inside containers. The supervisor
  never runs with raised privileges, so the flag bought nothing; without it the same question goes through the older
  call. Nothing in this landing tested inside such a container; the change only drops a flag the supervisor does not
  need, and the end-to-end runs above show the check still refuses what it should. Its comment, which still listed
  permission problems as host failures, now says why a folder the user cannot enter is treated as their choice to fix.

#### Things you should know that the report does not say

- **The restored tests' explanations are much thinner than the originals.** The test checking that an upload's
  acknowledgement is not stuck behind terminal output no longer explains why it measures priority rather than speed, and
  its waits shrank from 5 to 2 seconds without a reason given. The test capping a terminal's memory no longer says why
  it exists or how its 8 MiB limit was chosen. Possible follow-up: put the report's new measurements into those
  comments. A reviewer checked the upload test's premise still holds.
- The zero-size changelog fragment describes a request the desktop app never sends, so there is nothing in it for users;
  release-time curation can mark it as having no user-facing effect or drop it.
- Restarting a session whose folder cannot be entered now reports the folder problem before any problem with the saved
  conversation. No test covers that case.

#### Checks

The three plans change the desktop app's session list and launcher, the helm, the shared protocol, the supervisor and
its end-to-end tests. The landing ran the lints, the whole Rust workspace suite, the desktop process's tests, the app's
end-to-end smoke and the reference screenshots (goldens). Builds ran locally; the workspace suite and the smoke ran on
boat sandboxes (cloud VMs), from binaries built locally.

On the plans as delivered, stacked on main as it stood when the round started:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`,
  `python3 -B scripts/test-gui-e2e-smoke.py`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets -- -D warnings`
  once without and once with `--features agent`: passed.
- `cargo nextest run --workspace --exclude farhelm-desktop`, from a locally built archive, split in three across boats:
  2761, 2658 and 2636 tests, 8055 in all (`cc2d7e45-c34d-471b-9521-e292a5d3eabc`,
  `ad12af75-4c3f-479f-9c87-e8df09421a5a`, `89433db2-2d08-49cc-a1af-a5b8696a6edb`). One failed: the macro
  compile-contract test, which builds a small crate offline from this machine's cargo cache, which a boat does not have,
  so it fails there every time. It did not run anywhere this round; nothing in this round touches it, and it passed
  locally in an earlier round today. One test skipped itself at runtime because it needs a recreated file to get a new
  inode number and the boat's filesystem reused the old one; it did not run anywhere else this round.
- `cargo nextest run -p farhelm-desktop`, locally: 176 passed (`6610603b-7df3-43e5-ab7c-cf4756a7b544`).
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on a boat: 49 of its 50 steps passed and the clipboard
  paste step was skipped, as always on Linux (`a05cf1f2-6da9-4972-988a-f814bff20a5e`).
- `python3 scripts/check-gui-goldens.py`, locally: passed (`1c6c16a9-9ed6-4d42-9ce3-0399f6a2bc61`).

On the fixed tree (the Shift sweep's start threshold, the folder check's access call, comments and SPEC_impl.md):

- `cargo fmt --all -- --check`, `dprint check` and
  `cargo clippy --locked -p farhelm-gui -p farhelm-supervisor --all-targets -- -D warnings`: passed.
- The session list's whole test module, locally: 516 passed (`69a3bcc4-c0af-480b-9732-b36393d20617`), the sweep and grip
  drag tests included.
- The supervisor's session lifecycle and terminal tab end-to-end modules, which exercise the folder check on create,
  restart and tab open, locally: 96 passed (`a83797de-4468-4cb7-8c6d-df759c257c5b`).

The boat suites, the smoke, the desktop tests and the goldens were reused for the fixed tree: after them only the two
small code changes above and comments changed, and the focused runs above cover both.

Skipped:

- The Mac-only checks: no Mac-specific code changed.
