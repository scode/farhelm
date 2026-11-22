# Editing a full selection fits in the helm's queue

## What this was about

Pinning, unpinning, muting, unmuting, or changing the read status of a large selection could fill the helm's command
queue. The window would reconnect partway through the batch and lose the selection. The agreed fix gives the helm room
for 1,000 queued commands, twice the 500-session list limit. A single batch fits with room left over.

## Things you should know

A command sent from the window immediately after a large batch still waits behind the remaining session changes. A full
batch can contain up to 500 changes, and the helm applies commands in order. A busy session can refuse its edit while
the rest of the batch proceeds.

## Open questions and possible follow-ups

None.

## Pull requests

- [#4579](https://github.com/scode/farhelm/pull/4579/changes) raises the queue limit for bulk edits to marked sessions.

## Checks run, reused and skipped

- Ran the helm channel module with pinned nextest through the recorder.
  `cargo nextest run -p farhelm-helm --lib -E 'test(/^ui_channel::tests::/)'` passed all 44 cases on `5dda60de6c07` (run
  `af40a57f-a5d5-46ed-8d85-5a481c5b3317`). The regression keeps one command in progress, confirms the channel accepts
  1,000 more and answers a heartbeat, then confirms one more closes the channel. The final rebase added independent
  planning and TODO entries to the base. The tested code is identical.
- Reused the desktop tests `metadata_writes_go_once_per_eligible_row_and_clear_the_marks` and
  `a_busy_row_gets_the_not_started_line_until_its_next_operation` from the 46-test run on `97881a5f41a4` (run
  `93481083-7fea-474c-94fb-aba10679a9c6`). They exercise sending once per eligible session and refusing a busy row while
  other rows proceed. The fixture revision and upstream changes leave those desktop paths intact.
- Rust formatting, the test-delay checker, scoped Markdown formatting, changelog format, and whitespace checks passed.
- Skipped a full desktop run because the fix changes the helm's queue and focused cases cover the marked menu's
  behavior. Batch latency was unmeasured.

## Review gate

The source review cleared after two corrections. The test verifies the first command is held before filling the queue,
and the queue comment scopes the one-operation-per-session rule to edits that use the window's operation store. The
commit wording passed its cold read.
